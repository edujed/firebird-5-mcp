//! GraphQL (schema + resolvers) target.
//!
//! Generates:
//! - GraphQL schema (`.graphql`) with types, Query, and Mutation
//! - Node.js resolvers (CommonJS) with Firebird SQL queries

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

pub struct GraphqlTarget;

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

impl Target for GraphqlTarget {
    /// Maps a Firebird type to a GraphQL scalar type.
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::SmallInt | FbType::Integer | FbType::BigInt | FbType::Int128 => {
                "Int".to_string()
            }
            FbType::Float
            | FbType::DoublePrecision
            | FbType::Numeric { .. }
            | FbType::Decimal { .. }
            | FbType::DecFloat16
            | FbType::DecFloat34 => "Float".to_string(),
            FbType::Char { .. }
            | FbType::Varchar { .. }
            | FbType::Nvarchar { .. }
            | FbType::Cstring
            | FbType::BlobText => "String".to_string(),
            FbType::BlobBinary | FbType::Blob => "String".to_string(), // base64
            FbType::Date | FbType::Time | FbType::Timestamp => "String".to_string(), // ISO
            FbType::Boolean => "Boolean".to_string(),
            FbType::Unknown(_) => "String".to_string(),
        }
    }

    fn generate(
        &self,
        schema: &TableSchema,
        _fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> String {
        let type_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        // Package comment
        if let Some(pkg) = &opts.package {
            b.push_str(&format!("// Package: {}\n", pkg));
        }

        if opts.include_comments {
            b.push_str(&format!(
                "// GraphQL schema + resolvers for table {}\n\n",
                schema.table
            ));
        }

        // === SCHEMA SECTION ===
        b.push_str("=== SCHEMA (schema.graphql) ===\n\n");

        // Type definition
        b.push_str(&format!("type {} {{\n", type_name));
        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let gql_type = self.type_for(&ft);
            let non_null = if is_nullable(col) { "" } else { "!" };

            if opts.include_comments {
                if let Some(comment) = schema.comments.get(&col.name) {
                    b.push_str(&format!("  \"{}\"\n", comment));
                }
            }

            b.push_str(&format!("  {} {}{}\n", field_name, gql_type, non_null));
        }
        b.push_str("}\n\n");

        // Query type
        b.push_str("type Query {\n");
        // Single row by PK
        let pk_cols = find_pk_columns(schema);
        let pk_col_names: Vec<String> = pk_cols.iter().map(|c| c.name.clone()).collect();
        if !pk_cols.is_empty() {
            let pk_args: Vec<String> = pk_cols
                .iter()
                .map(|col| {
                    let ft = FbType::from_column(col);
                    let arg_name = to_camel_case(&col.name);
                    let gql_type = self.type_for(&ft);
                    format!("{}: {}!", arg_name, gql_type)
                })
                .collect();
            b.push_str(&format!(
                "  {}({}): {}\n",
                to_camel_case(&schema.table),
                pk_args.join(", "),
                type_name
            ));
        }
        // List all
        b.push_str(&format!(
            "  {}s: [{}!]!\n",
            to_camel_case(&schema.table),
            type_name
        ));
        b.push_str("}\n\n");

        // Mutation type (only for tables, not views)
        if !opts.is_view {
            b.push_str("type Mutation {\n");

            // Create
            let create_args: Vec<String> = schema
                .columns
                .iter()
                .filter(|col| !pk_col_names.contains(&col.name)) // exclude PK (auto-generated)
                .map(|col| {
                    let ft = FbType::from_column(col);
                    let arg_name = to_camel_case(&col.name);
                    let gql_type = self.type_for(&ft);
                    let non_null = if is_nullable(col) { "" } else { "!" };
                    format!("{}: {}{}", arg_name, gql_type, non_null)
                })
                .collect();
            b.push_str(&format!(
                "  criar{}({}): {}\n",
                type_name,
                create_args.join(", "),
                type_name
            ));

            // Update
            let update_args: Vec<String> = {
                let mut args: Vec<String> = pk_cols
                    .iter()
                    .map(|col| {
                        let ft = FbType::from_column(col);
                        let arg_name = to_camel_case(&col.name);
                        let gql_type = self.type_for(&ft);
                        format!("{}: {}!", arg_name, gql_type)
                    })
                    .collect();
                args.extend(
                    schema
                        .columns
                        .iter()
                        .filter(|col| !pk_col_names.contains(&col.name))
                        .map(|col| {
                            let ft = FbType::from_column(col);
                            let arg_name = to_camel_case(&col.name);
                            let gql_type = self.type_for(&ft);
                            format!("{}: {}", arg_name, gql_type)
                        }),
                );
                args
            };
            b.push_str(&format!(
                "  atualizar{}({}): {}\n",
                type_name,
                update_args.join(", "),
                type_name
            ));

            // Delete
            let delete_args: Vec<String> = pk_cols
                .iter()
                .map(|col| {
                    let ft = FbType::from_column(col);
                    let arg_name = to_camel_case(&col.name);
                    let gql_type = self.type_for(&ft);
                    format!("{}: {}!", arg_name, gql_type)
                })
                .collect();
            b.push_str(&format!(
                "  deletar{}({}): Boolean\n",
                type_name,
                delete_args.join(", ")
            ));

            b.push_str("}\n");
        }

        // === RESOLVERS SECTION ===
        b.push_str("\n=== RESOLVERS (resolvers.js) ===\n\n");
        b.push_str("// Node.js resolvers for Apollo Server\n");
        b.push_str("// Requires: @apollo/server, express, and a Firebird DB wrapper\n\n");
        b.push_str("// const { query, exec } = require('../db'); // your Firebird wrapper\n\n");

        // Query resolvers
        b.push_str("exports.Query = {\n");
        if !pk_cols.is_empty() {
            let pk_params: Vec<String> =
                pk_cols.iter().map(|col| to_camel_case(&col.name)).collect();
            b.push_str(&format!(
                "  {}: async (_, {{ {} }}) => {{\n",
                to_camel_case(&schema.table),
                pk_params.join(", ")
            ));
            b.push_str(&format!(
                "    const rows = await query('SELECT * FROM {} WHERE {}', [{}]);\n",
                schema.table,
                pk_cols
                    .iter()
                    .map(|col| format!("{} = ?", col.name))
                    .collect::<Vec<_>>()
                    .join(" AND "),
                pk_params.join(", ")
            ));
            b.push_str("    return rows[0] || null;\n");
            b.push_str("  },\n");
        }
        b.push_str(&format!(
            "  {}s: async () => {{\n",
            to_camel_case(&schema.table)
        ));
        b.push_str(&format!(
            "    return await query('SELECT * FROM {}');\n",
            schema.table
        ));
        b.push_str("  },\n");
        b.push_str("};\n\n");

        // Mutation resolvers (only for tables, not views)
        if !opts.is_view {
            b.push_str("exports.Mutation = {\n");

            // Create
            let insert_cols: Vec<String> = schema
                .columns
                .iter()
                .filter(|col| !pk_col_names.contains(&col.name))
                .map(|col| col.name.clone())
                .collect();
            let insert_placeholders: Vec<String> =
                insert_cols.iter().map(|_| "?".to_string()).collect();
            let insert_params: Vec<String> = schema
                .columns
                .iter()
                .filter(|col| !pk_col_names.contains(&col.name))
                .map(|col| to_camel_case(&col.name))
                .collect();

            b.push_str(&format!(
                "  criar{}: async (_, {{ {} }}) => {{\n",
                type_name,
                insert_params.join(", ")
            ));
            b.push_str(&format!(
                "    await exec('INSERT INTO {} ({}) VALUES ({})', [{}]);\n",
                schema.table,
                insert_cols.join(", "),
                insert_placeholders.join(", "),
                insert_params.join(", ")
            ));
            // Return the created row (by PK if available, else by first non-null field)
            if !pk_cols.is_empty() {
                b.push_str(&format!(
                    "    // TODO: return the created row (query by PK or use sequence)\n"
                ));
            }
            b.push_str("    return null;\n");
            b.push_str("  },\n");

            // Update
            let update_sets: Vec<String> = schema
                .columns
                .iter()
                .filter(|col| !pk_col_names.contains(&col.name))
                .map(|col| format!("{} = ?", col.name))
                .collect();
            let update_params: Vec<String> = schema
                .columns
                .iter()
                .filter(|col| !pk_col_names.contains(&col.name))
                .map(|col| to_camel_case(&col.name))
                .collect();
            let update_pk_params: Vec<String> =
                pk_cols.iter().map(|col| to_camel_case(&col.name)).collect();

            b.push_str(&format!(
                "  atualizar{}: async (_, {{ {}, {} }}) => {{\n",
                type_name,
                update_pk_params.join(", "),
                update_params.join(", ")
            ));
            b.push_str(&format!(
                "    await exec('UPDATE {} SET {} WHERE {}', [{}]);\n",
                schema.table,
                update_sets.join(", "),
                pk_cols
                    .iter()
                    .map(|col| format!("{} = ?", col.name))
                    .collect::<Vec<_>>()
                    .join(" AND "),
                [update_params, update_pk_params.clone()]
                    .concat()
                    .join(", ")
            ));
            b.push_str("    return null;\n");
            b.push_str("  },\n");

            // Delete
            b.push_str(&format!(
                "  deletar{}: async (_, {{ {} }}) => {{\n",
                type_name,
                update_pk_params.join(", ")
            ));
            b.push_str(&format!(
                "    await exec('DELETE FROM {} WHERE {}', [{}]);\n",
                schema.table,
                pk_cols
                    .iter()
                    .map(|col| format!("{} = ?", col.name))
                    .collect::<Vec<_>>()
                    .join(" AND "),
                update_pk_params.join(", ")
            ));
            b.push_str("    return true;\n");
            b.push_str("  },\n");

            b.push_str("};\n");
        }

        b
    }
}

/// Finds the primary key columns for a table.
fn find_pk_columns(schema: &TableSchema) -> Vec<&ColumnInfo> {
    // Prefer PRIMARY KEY constraint
    if let Some(pk_constraint) = schema
        .constraints
        .iter()
        .find(|c| c.r#type.eq_ignore_ascii_case("PRIMARY KEY"))
    {
        // Find the index matching the PK constraint name
        if let Some(idx) = schema.indexes.iter().find(|i| i.name == pk_constraint.name) {
            let pk_col_names: Vec<String> = idx
                .columns
                .split(',')
                .map(|c| c.trim().to_string())
                .collect();
            return schema
                .columns
                .iter()
                .filter(|col| pk_col_names.contains(&col.name))
                .collect();
        }
    }

    // Fallback: first unique index
    if let Some(idx) = schema.indexes.iter().find(|i| i.unique) {
        let pk_col_names: Vec<String> = idx
            .columns
            .split(',')
            .map(|c| c.trim().to_string())
            .collect();
        return schema
            .columns
            .iter()
            .filter(|col| pk_col_names.contains(&col.name))
            .collect();
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Language;
    use crate::schema::{ColumnInfo, ConstraintInfo, IndexInfo, TableSchema};

    fn col(name: &str, type_num: i64, sub_type: i64, size: i64, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            type_num,
            sub_type,
            size,
            precision: 0,
            scale: 0,
            nullable: Some(if nullable { 1 } else { 0 }),
            default: None,
            position: None,
            domain: String::new(),
        }
    }

    fn test_schema() -> TableSchema {
        TableSchema {
            table: "USUARIO".to_string(),
            columns: vec![
                col("ID", 8, 0, 0, false),
                col("NOME", 272, 0, 100, true),
                col("EMAIL", 272, 0, 200, true),
                col("SALARIO", 16, 2, 0, false),
                col("CRIADO_EM", 35, 0, 0, false),
            ],
            indexes: vec![IndexInfo {
                name: "PK_USUARIO".to_string(),
                unique: true,
                r#type: Some(0),
                columns: "ID".to_string(),
            }],
            constraints: vec![ConstraintInfo {
                name: "PK_USUARIO".to_string(),
                r#type: "PRIMARY KEY".to_string(),
            }],
            triggers: vec![],
            comments: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn type_for_integer_family() {
        let target = GraphqlTarget;
        assert_eq!(target.type_for(&FbType::SmallInt), "Int");
        assert_eq!(target.type_for(&FbType::Integer), "Int");
        assert_eq!(target.type_for(&FbType::BigInt), "Int");
        assert_eq!(target.type_for(&FbType::Int128), "Int");
    }

    #[test]
    fn type_for_float_family() {
        let target = GraphqlTarget;
        assert_eq!(target.type_for(&FbType::Float), "Float");
        assert_eq!(target.type_for(&FbType::DoublePrecision), "Float");
        assert_eq!(
            target.type_for(&FbType::Numeric {
                precision: 10,
                scale: 2
            }),
            "Float"
        );
        assert_eq!(
            target.type_for(&FbType::Decimal {
                precision: 10,
                scale: 2
            }),
            "Float"
        );
    }

    #[test]
    fn type_for_string_family() {
        let target = GraphqlTarget;
        assert_eq!(target.type_for(&FbType::Varchar { size: 100 }), "String");
        assert_eq!(target.type_for(&FbType::Char { size: 10 }), "String");
        assert_eq!(target.type_for(&FbType::Nvarchar { size: 100 }), "String");
        assert_eq!(target.type_for(&FbType::BlobText), "String");
    }

    #[test]
    fn type_for_temporal_and_boolean() {
        let target = GraphqlTarget;
        assert_eq!(target.type_for(&FbType::Date), "String");
        assert_eq!(target.type_for(&FbType::Time), "String");
        assert_eq!(target.type_for(&FbType::Timestamp), "String");
        assert_eq!(target.type_for(&FbType::Boolean), "Boolean");
    }

    #[test]
    fn generate_includes_type_definition() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Graphql, false);
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("type Usuario {"));
        // ID is nullable in test_schema, so it should be "id Int"
        assert!(output.contains("id Int"), "Missing 'id Int' in output");
        // NOME is nullable, so it should be "nome String"
        assert!(
            output.contains("nome String"),
            "Missing 'nome String' in output"
        );
        // EMAIL is nullable, so it should be "email String"
        assert!(
            output.contains("email String"),
            "Missing 'email String' in output"
        );
        // SALARIO is NOT NULL, so it should be "salario Float!"
        assert!(
            output.contains("salario Float!"),
            "Missing 'salario Float!' in output"
        );
        // CRIADO_EM is NOT NULL, so it should be "criadoEm String!"
        assert!(
            output.contains("criadoEm String!"),
            "Missing 'criadoEm String!' in output"
        );
    }

    #[test]
    fn generate_includes_query_type() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Graphql, false);
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("type Query {"));
        assert!(output.contains("usuario(id: Int!): Usuario"));
        assert!(output.contains("usuarios: [Usuario!]!"));
    }

    #[test]
    fn generate_includes_mutation_type_for_table() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Graphql, false);
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("type Mutation {"));
        assert!(output.contains("criarUsuario("));
        assert!(output.contains("atualizarUsuario(id: Int!"));
        assert!(output.contains("deletarUsuario(id: Int!): Boolean"));
    }

    #[test]
    fn generate_suppresses_mutation_for_view() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let mut opts = GenOptions::new(Language::Graphql, false);
        opts.is_view = true;
        let output = target.generate(&schema, &[], &opts);

        assert!(!output.contains("type Mutation {"));
        assert!(!output.contains("criarUsuario"));
        assert!(!output.contains("atualizarUsuario"));
        assert!(!output.contains("deletarUsuario"));
    }

    #[test]
    fn generate_includes_resolvers() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Graphql, false);
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("exports.Query = {"));
        assert!(output.contains("exports.Mutation = {"));
        assert!(output.contains("SELECT * FROM USUARIO WHERE ID = ?"));
        assert!(output.contains("SELECT * FROM USUARIO"));
        assert!(output.contains("INSERT INTO USUARIO"));
        assert!(output.contains("UPDATE USUARIO SET"));
        assert!(output.contains("DELETE FROM USUARIO WHERE ID = ?"));
    }

    #[test]
    fn generate_update_resolver_flat_params() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Graphql, false);
        let output = target.generate(&schema, &[], &opts);

        // Debug: print the UPDATE line
        let update_line = output
            .lines()
            .find(|l| l.contains("UPDATE USUARIO SET"))
            .unwrap_or("NOT FOUND");
        // Must NOT have nested array [[...]]
        assert!(
            !output.contains("[["),
            "Found nested array in UPDATE resolver: {}",
            output
        );
        // Must have flat array [cols, pk] (matching SQL: SET cols WHERE pk)
        // Test schema has: NOME, EMAIL, SALARIO, CRIADO_EM (non-PK) + ID (PK)
        assert!(output.contains("[nome, email, salario, criadoEm, id]"));
    }

    #[test]
    fn generate_suppresses_mutation_resolvers_for_view() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let mut opts = GenOptions::new(Language::Graphql, false);
        opts.is_view = true;
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("exports.Query = {"));
        assert!(!output.contains("exports.Mutation = {"));
        assert!(!output.contains("INSERT INTO"));
        assert!(!output.contains("UPDATE"));
        assert!(!output.contains("DELETE FROM"));
    }

    #[test]
    fn generate_includes_package_comment() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let mut opts = GenOptions::new(Language::Graphql, false);
        opts.package = Some("com.app.graphql".to_string());
        let output = target.generate(&schema, &[], &opts);

        assert!(output.contains("// Package: com.app.graphql"));
    }

    #[test]
    fn generate_respects_include_comments() {
        let target = GraphqlTarget;
        let schema = test_schema();
        let mut opts = GenOptions::new(Language::Graphql, false);
        opts.include_comments = false;
        let output = target.generate(&schema, &[], &opts);

        assert!(!output.contains("// GraphQL schema + resolvers for table"));
    }

    #[test]
    fn find_pk_columns_uses_primary_key_constraint() {
        let schema = test_schema();
        let pk_cols = find_pk_columns(&schema);

        assert_eq!(pk_cols.len(), 1);
        assert_eq!(pk_cols[0].name, "ID");
    }

    #[test]
    fn find_pk_columns_falls_back_to_unique_index() {
        let mut schema = test_schema();
        schema.constraints.clear();
        let pk_cols = find_pk_columns(&schema);

        assert_eq!(pk_cols.len(), 1);
        assert_eq!(pk_cols[0].name, "ID");
    }

    #[test]
    fn find_pk_columns_returns_empty_when_no_pk() {
        let mut schema = test_schema();
        schema.constraints.clear();
        schema.indexes.clear();
        let pk_cols = find_pk_columns(&schema);

        assert!(pk_cols.is_empty());
    }
}
