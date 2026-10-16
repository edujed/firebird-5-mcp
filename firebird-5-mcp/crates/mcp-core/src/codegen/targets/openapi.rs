//! OpenAPI 3.2.0 target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

pub struct OpenApiTarget;

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

impl Target for OpenApiTarget {
    /// Maps a Firebird type to `(type, format)` for OpenAPI.
    fn type_for(&self, t: &FbType) -> String {
        // The OpenAPI target needs both a type and an optional format; the
        // trait's single-string `type_for` returns the type, and the format is
        // derived in `generate` via `openapi_format`.
        openapi_type(t).0
    }

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], include_fks: bool) -> String {
        let schema_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str(&format!("  {}:\n", schema_name));
        b.push_str("    type: object\n");
        b.push_str(&format!(
            "    description: Schema for table {}\n",
            schema.table
        ));

        // Table comment
        if let Some(table_comment) = schema.comments.get("table") {
            b.push_str(&format!("    description: {}\n", table_comment));
        }

        // Required fields (NOT NULL)
        let required: Vec<String> = schema
            .columns
            .iter()
            .filter(|c| !is_nullable(c))
            .map(|c| to_camel_case(&c.name))
            .collect();
        if !required.is_empty() {
            b.push_str("    required:\n");
            for field in &required {
                b.push_str(&format!("      - {}\n", field));
            }
        }

        b.push_str("    properties:\n");

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let (type_str, format_str) = openapi_type(&ft);
            let nullable = is_nullable(col);

            b.push_str(&format!("      {}:\n", field_name));
            b.push_str(&format!("        type: {}\n", type_str));
            if let Some(fmt) = &format_str {
                b.push_str(&format!("        format: {}\n", fmt));
            }
            if nullable {
                b.push_str("        nullable: true\n");
            }

            // Description from comments
            if let Some(comment) = schema.comments.get(&col.name) {
                b.push_str(&format!("        description: {}\n", comment));
            }

            // FK reference
            if include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    b.push_str(&format!(
                        "        x-foreign-key: {}.{}\n",
                        fk.ref_table, fk.ref_column
                    ));
                }
            }
        }

        b
    }
}

fn openapi_type(t: &FbType) -> (String, Option<String>) {
    match t {
        FbType::BlobText => ("string".to_string(), None),
        FbType::BlobBinary | FbType::Blob => ("string".to_string(), Some("binary".to_string())),
        FbType::Numeric { .. } | FbType::Decimal { .. } => {
            ("number".to_string(), Some("decimal".to_string()))
        }
        FbType::Float | FbType::DoublePrecision => {
            ("number".to_string(), Some("double".to_string()))
        }
        FbType::SmallInt | FbType::Integer | FbType::BigInt | FbType::Int128 => {
            ("integer".to_string(), Some("int32".to_string()))
        }
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
