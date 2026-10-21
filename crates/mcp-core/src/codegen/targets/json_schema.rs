//! JSON Schema (draft 2020-12) target.
//!
//! Each table produces one `.json` file (format `json-schema`) containing a
//! JSON Schema object describing the table's columns. The schema uses
//! Firebird-specific extensions (`x-numeric`, `x-column-order`,
//! `x-foreign-key`) to preserve information that standard JSON Schema cannot
//! express (NUMERIC/DECIMAL precision/scale, column order, FK references).
//!
//! Type mapping:
//! - `NUMERIC`/`DECIMAL` → `string` + `x-numeric: {precision, scale}`
//! - `CHAR`/`VARCHAR`/`NVARCHAR` → `string` + `maxLength`
//! - `DATE`/`TIME`/`TIMESTAMP` → `string` + `format: date|time|date-time`
//! - `BLOB SUB_TYPE BINARY` → `string` + `format: binary`
//! - `BOOLEAN` → `boolean`
//! - `SMALLINT`/`INTEGER` → `integer` + `format: int32`
//! - `BIGINT` → `integer` + `format: int64`
//! - `INT128` → `string`
//! - `FLOAT`/`DOUBLE PRECISION`/`DECFLOAT` → `number` + `format: double`

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::schema::{ForeignKeyInfo, TableSchema};

pub struct JsonSchemaTarget;

/// Escapes a string for inclusion in a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

impl Target for JsonSchemaTarget {
    /// Maps a Firebird type to the JSON Schema `type` keyword.
    fn type_for(&self, t: &FbType) -> String {
        json_schema_type(t).0
    }

    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let mut b = String::new();
        b.push_str("{\n");
        b.push_str("  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n");
        b.push_str(&format!(
            "  \"$id\": \"https://firebird-mcp/schemas/{}.json\",\n",
            schema.table
        ));
        b.push_str(&format!("  \"title\": \"{}\",\n", schema.table));

        // Table comment (or default description)
        if opts.include_comments {
            if let Some(table_comment) = schema.comments.get("table") {
                b.push_str(&format!(
                    "  \"description\": \"{}\",\n",
                    json_escape(table_comment)
                ));
            } else {
                b.push_str(&format!(
                    "  \"description\": \"Schema for table {}\",\n",
                    schema.table
                ));
            }
        }

        b.push_str("  \"type\": \"object\",\n");

        // Required fields (NOT NULL) — original column names
        let required: Vec<&str> = schema
            .columns
            .iter()
            .filter(|c| !is_nullable(c))
            .map(|c| c.name.as_str())
            .collect();
        if !required.is_empty() {
            b.push_str("  \"required\": [");
            for (i, field) in required.iter().enumerate() {
                if i > 0 {
                    b.push_str(", ");
                }
                b.push('"');
                b.push_str(field);
                b.push('"');
            }
            b.push_str("],\n");
        }

        // Column order (always emitted)
        b.push_str("  \"x-column-order\": [");
        for (i, col) in schema.columns.iter().enumerate() {
            if i > 0 {
                b.push_str(", ");
            }
            b.push('"');
            b.push_str(&col.name);
            b.push('"');
        }
        b.push_str("],\n");

        b.push_str("  \"properties\": {\n");
        for (i, col) in schema.columns.iter().enumerate() {
            let ft = FbType::from_column(col);
            let (type_str, format_str) = json_schema_type(&ft);
            let nullable = is_nullable(col);

            b.push_str(&format!("    \"{}\": {{\n", col.name));
            b.push_str(&format!("      \"type\": \"{}\"", type_str));
            if let Some(fmt) = &format_str {
                b.push_str(&format!(",\n      \"format\": \"{}\"", fmt));
            }

            // maxLength for character types
            let size = ft.char_size();
            if size > 0 {
                b.push_str(&format!(",\n      \"maxLength\": {}", size));
            }

            // x-numeric for NUMERIC/DECIMAL
            if let Some((precision, scale)) = ft.precision_scale() {
                b.push_str(&format!(
                    ",\n      \"x-numeric\": {{ \"precision\": {}, \"scale\": {} }}",
                    precision, scale
                ));
            }

            // nullable
            if nullable {
                b.push_str(",\n      \"nullable\": true");
            }

            // Description from comments
            if opts.include_comments {
                if let Some(comment) = schema.comments.get(&col.name) {
                    b.push_str(&format!(
                        ",\n      \"description\": \"{}\"",
                        json_escape(comment)
                    ));
                }
            }

            // FK reference
            if opts.include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    b.push_str(&format!(
                        ",\n      \"x-foreign-key\": \"{}.{}\"",
                        fk.ref_table, fk.ref_column
                    ));
                }
            }

            b.push_str("\n    }");
            if i + 1 < schema.columns.len() {
                b.push_str(",");
            }
            b.push_str("\n");
        }
        b.push_str("  }\n");
        b.push_str("}\n");

        vec![GeneratedFile::new(
            format!("{}.json", schema.table.to_lowercase()),
            b,
            "json-schema",
        )]
    }
}

/// Maps a Firebird type to `(type, format)` for JSON Schema.
fn json_schema_type(t: &FbType) -> (String, Option<String>) {
    match t {
        FbType::BlobText => ("string".to_string(), None),
        FbType::BlobBinary | FbType::Blob => ("string".to_string(), Some("binary".to_string())),
        FbType::Numeric { .. } | FbType::Decimal { .. } => ("string".to_string(), None),
        FbType::Float | FbType::DoublePrecision => {
            ("number".to_string(), Some("double".to_string()))
        }
        FbType::SmallInt => ("integer".to_string(), Some("int32".to_string())),
        FbType::Integer => ("integer".to_string(), Some("int32".to_string())),
        FbType::BigInt => ("integer".to_string(), Some("int64".to_string())),
        FbType::Int128 => ("string".to_string(), None),
        FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
            ("string".to_string(), None)
        }
        FbType::Cstring => ("string".to_string(), None),
        FbType::Date => ("string".to_string(), Some("date".to_string())),
        FbType::Time => ("string".to_string(), Some("time".to_string())),
        FbType::Timestamp => ("string".to_string(), Some("date-time".to_string())),
        FbType::Boolean => ("boolean".to_string(), None),
        FbType::DecFloat16 | FbType::DecFloat34 => {
            ("number".to_string(), Some("double".to_string()))
        }
        FbType::Unknown(_) => ("string".to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Language;
    use crate::schema::{ColumnInfo, ConstraintInfo, IndexInfo};

    fn col(name: &str, type_num: i64, sub_type: i64, size: i64, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            type_num,
            sub_type,
            size,
            precision: 0,
            scale: 0,
            nullable: if nullable { None } else { Some(1) },
            default: None,
            position: None,
            domain: String::new(),
        }
    }

    fn col_prec(
        name: &str,
        type_num: i64,
        sub_type: i64,
        precision: i64,
        scale: i64,
        nullable: bool,
    ) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            type_num,
            sub_type,
            size: 0,
            precision,
            scale,
            nullable: if nullable { None } else { Some(1) },
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
                col("NOME", 20, 0, 100, true),
                col("EMAIL", 20, 0, 200, true),
                col_prec("SALARIO", 16, 2, 10, 2, false),
                col("CRIADO_EM", 29, 0, 0, false),
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

    fn content(files: &[GeneratedFile]) -> &str {
        files
            .iter()
            .find(|f| f.format == "json-schema")
            .map(|f| f.content.as_str())
            .unwrap_or("")
    }

    #[test]
    fn type_for_integer_family() {
        let t = JsonSchemaTarget;
        assert_eq!(t.type_for(&FbType::SmallInt), "integer");
        assert_eq!(t.type_for(&FbType::Integer), "integer");
        assert_eq!(t.type_for(&FbType::BigInt), "integer");
        assert_eq!(t.type_for(&FbType::Int128), "string");
    }

    #[test]
    fn type_for_numeric_family() {
        let t = JsonSchemaTarget;
        assert_eq!(
            t.type_for(&FbType::Numeric {
                precision: 10,
                scale: 2
            }),
            "string"
        );
        assert_eq!(
            t.type_for(&FbType::Decimal {
                precision: 15,
                scale: 4
            }),
            "string"
        );
        assert_eq!(t.type_for(&FbType::DecFloat16), "number");
        assert_eq!(t.type_for(&FbType::DecFloat34), "number");
    }

    #[test]
    fn type_for_string_family() {
        let t = JsonSchemaTarget;
        assert_eq!(t.type_for(&FbType::Char { size: 40 }), "string");
        assert_eq!(t.type_for(&FbType::Varchar { size: 100 }), "string");
        assert_eq!(t.type_for(&FbType::Nvarchar { size: 200 }), "string");
        assert_eq!(t.type_for(&FbType::Cstring), "string");
        assert_eq!(t.type_for(&FbType::BlobText), "string");
    }

    #[test]
    fn type_for_temporal_and_boolean() {
        let t = JsonSchemaTarget;
        assert_eq!(t.type_for(&FbType::Date), "string");
        assert_eq!(t.type_for(&FbType::Time), "string");
        assert_eq!(t.type_for(&FbType::Timestamp), "string");
        assert_eq!(t.type_for(&FbType::Boolean), "boolean");
    }

    #[test]
    fn generate_returns_single_file() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "usuario.json");
        assert_eq!(files[0].format, "json-schema");
    }

    #[test]
    fn generate_includes_schema_header() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"$schema\": \"https://json-schema.org/draft/2020-12/schema\""));
        assert!(c.contains("\"$id\": \"https://firebird-mcp/schemas/USUARIO.json\""));
        assert!(c.contains("\"title\": \"USUARIO\""));
        assert!(c.contains("\"type\": \"object\""));
    }

    #[test]
    fn generate_includes_required_fields() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"required\": [\"ID\", \"SALARIO\", \"CRIADO_EM\"]"));
    }

    #[test]
    fn generate_includes_column_order() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains(
            "\"x-column-order\": [\"ID\", \"NOME\", \"EMAIL\", \"SALARIO\", \"CRIADO_EM\"]"
        ));
    }

    #[test]
    fn generate_includes_max_length() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"maxLength\": 100"));
        assert!(c.contains("\"maxLength\": 200"));
    }

    #[test]
    fn generate_includes_numeric_extension() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"x-numeric\": { \"precision\": 10, \"scale\": 2 }"));
    }

    #[test]
    fn generate_includes_formats() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"format\": \"int32\""));
        assert!(c.contains("\"format\": \"date-time\""));
    }

    #[test]
    fn generate_includes_nullable() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let files = t.generate(&test_schema(), &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"nullable\": true"));
    }

    #[test]
    fn generate_fk_emits_foreign_key_extension() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, true);
        let fks = vec![ForeignKeyInfo {
            constraint_name: "FK_PESSOA".to_string(),
            table: "USUARIO".to_string(),
            column: "PESSOA_ID".to_string(),
            ref_table: "PESSOA".to_string(),
            ref_column: "ID".to_string(),
        }];
        let schema = TableSchema {
            table: "USUARIO".to_string(),
            columns: vec![col("ID", 8, 0, 0, false), col("PESSOA_ID", 8, 0, 0, true)],
            indexes: vec![],
            constraints: vec![],
            triggers: vec![],
            comments: std::collections::HashMap::new(),
        };
        let files = t.generate(&schema, &[&fks[0]], &opts);
        let c = content(&files);
        assert!(c.contains("\"x-foreign-key\": \"PESSOA.ID\""));
    }

    #[test]
    fn generate_view_suppresses_required() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let schema = TableSchema {
            table: "V_USUARIO".to_string(),
            columns: vec![col("ID", 8, 0, 0, true), col("NOME", 20, 0, 100, true)],
            indexes: vec![],
            constraints: vec![],
            triggers: vec![],
            comments: std::collections::HashMap::new(),
        };
        let files = t.generate(&schema, &[], &opts);
        let c = content(&files);
        assert!(!c.contains("\"required\""));
    }

    #[test]
    fn generate_includes_comments() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let mut schema = test_schema();
        schema
            .comments
            .insert("table".to_string(), "Tabela de usuários".to_string());
        schema
            .comments
            .insert("ID".to_string(), "Identificador".to_string());
        let files = t.generate(&schema, &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"description\": \"Tabela de usuários\""));
        assert!(c.contains("\"description\": \"Identificador\""));
    }

    #[test]
    fn generate_escapes_special_chars() {
        let t = JsonSchemaTarget;
        let opts = GenOptions::new(Language::JsonSchema, false);
        let mut schema = test_schema();
        schema.comments.insert(
            "table".to_string(),
            "Tabela com \"aspas\" e \\ backslash".to_string(),
        );
        let files = t.generate(&schema, &[], &opts);
        let c = content(&files);
        assert!(c.contains("\"description\": \"Tabela com \\\"aspas\\\" e \\\\ backslash\""));
    }
}
