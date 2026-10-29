//! Rust (sqlx) target.

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_pascal_case, to_snake_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

/// Rust code generation target (sqlx-compatible structs).
pub struct RustTarget;

/// Implements the `Target` trait for Rust code generation.
impl Target for RustTarget {
    /// Maps a Firebird type to a Rust type (sqlx-compatible).
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "String".to_string(),
            FbType::BlobBinary | FbType::Blob => "Vec<u8>".to_string(),
            FbType::Numeric { .. } | FbType::Decimal { .. } => "Decimal".to_string(),
            FbType::Float | FbType::DoublePrecision => "f64".to_string(),
            FbType::SmallInt => "i16".to_string(),
            FbType::Integer => "i32".to_string(),
            FbType::BigInt => "i64".to_string(),
            FbType::Int128 => "i128".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "String".to_string()
            }
            FbType::Cstring => "String".to_string(),
            FbType::Date => "NaiveDate".to_string(),
            FbType::Time => "NaiveTime".to_string(),
            FbType::Timestamp => "NaiveDateTime".to_string(),
            FbType::Boolean => "bool".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "Decimal".to_string(),
            FbType::Unknown(_) => "i64".to_string(),
        }
    }

    /// Returns the nullable Rust type (Option wrapper) for a Firebird type.
    fn dto_type_for(&self, t: &FbType) -> String {
        format!("Option<{}>", self.type_for(t))
    }

    /// Generates a Rust struct with sqlx FromRow derive for the table.
    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let struct_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        if opts.include_comments {
            b.push_str(&format!(
                "// {} maps to table {}\n",
                struct_name, schema.table
            ));
        }

        // Emit required imports based on column types
        let imports = rust_imports(schema);
        if !imports.is_empty() {
            for imp in &imports {
                b.push_str(&format!("use {}\n", imp));
            }
            b.push('\n');
        }

        b.push_str("#[derive(Debug, Clone, FromRow)]\n");
        b.push_str(&format!("pub struct {} {{\n", struct_name));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let base_type = self.type_for(&ft);
            let nullable = is_nullable(col);

            if opts.include_comments {
                let mut comment = format!("    /// Column: {}", col.name);
                if opts.include_fks {
                    if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                        comment.push_str(&format!(" — FK → {}.{}", fk.ref_table, fk.ref_column));
                    }
                }
                comment.push('\n');
                b.push_str(&comment);
            }

            let type_str = if nullable {
                format!("Option<{}>", base_type)
            } else {
                base_type
            };

            b.push_str(&format!("    pub {}: {},\n", field_name, type_str));
        }

        b.push_str("}\n");

        // Example query with aliases matching field names
        if opts.include_examples {
            b.push_str("\n// Example query (aliases must match field names for FromRow):\n");
            let aliases: Vec<String> = schema
                .columns
                .iter()
                .map(|col| format!("{} AS {}", col.name, to_snake_case(&col.name)))
                .collect();
            b.push_str(&format!(
                "// SELECT {} FROM {}\n",
                aliases.join(", "),
                schema.table
            ));
            b.push_str(&format!(
                "// let rows: Vec<{}> = conn.query(sql, ({{}}))?;\n",
                struct_name
            ));
            b.push_str(&format!(
                "// let rows: Vec<{}> = conn.query(sql, ({{}}))?;\n",
                struct_name
            ));
        }

        vec![GeneratedFile::new(
            format!("{}.rs", struct_name.to_lowercase()),
            b,
            "rust",
        )]
    }
}

/// Returns the `use` statements needed for the types used in this table.
fn rust_imports(schema: &TableSchema) -> Vec<&'static str> {
    let mut imports: Vec<&'static str> = Vec::new();
    imports.push("sqlx::FromRow");

    for col in &schema.columns {
        let ft = FbType::from_column(col);
        match ft {
            FbType::Date => imports.push("chrono::NaiveDate"),
            FbType::Time => imports.push("chrono::NaiveTime"),
            FbType::Timestamp => imports.push("chrono::NaiveDateTime"),
            FbType::Numeric { .. }
            | FbType::Decimal { .. }
            | FbType::DecFloat16
            | FbType::DecFloat34 => imports.push("bigdecimal::BigDecimal"),
            _ => {}
        }
    }

    imports.sort();
    imports.dedup();
    imports
}
