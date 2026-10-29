//! OpenAPI 3.2.0 target.

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

/// OpenAPI 3.2.0 code generation target.
pub struct OpenApiTarget;

/// Implements the `Target` trait for OpenAPI code generation.
impl Target for OpenApiTarget {
    /// Maps a Firebird type to `(type, format)` for OpenAPI.
    fn type_for(&self, t: &FbType) -> String {
        // The OpenAPI target needs both a type and an optional format; the
        // trait's single-string `type_for` returns the type, and the format is
        // derived in `generate` via `openapi_format`.
        openapi_type(t).0
    }

    /// Generates an OpenAPI 3.2.0 schema for the table.
    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let schema_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str(&format!("  {}:\n", schema_name));
        b.push_str("    type: object\n");

        // Table comment (or default description)
        if opts.include_comments {
            if let Some(table_comment) = schema.comments.get("table") {
                b.push_str(&format!("    description: {}\n", table_comment));
            } else {
                b.push_str(&format!(
                    "    description: Schema for table {}\n",
                    schema.table
                ));
            }
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
            if opts.include_comments {
                if let Some(comment) = schema.comments.get(&col.name) {
                    b.push_str(&format!("        description: {}\n", comment));
                }
            }

            // FK reference
            if opts.include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    b.push_str(&format!(
                        "        x-foreign-key: {}.{}\n",
                        fk.ref_table, fk.ref_column
                    ));
                }
            }
        }

        vec![GeneratedFile::new(
            format!("{}.yaml", schema.table.to_lowercase()),
            b,
            "openapi",
        )]
    }
}

/// Maps a Firebird type to an OpenAPI `(type, format)` pair.
fn openapi_type(t: &FbType) -> (String, Option<String>) {
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
