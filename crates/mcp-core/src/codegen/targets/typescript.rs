//! TypeScript target.

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

/// TypeScript code generation target (interfaces + types).
pub struct TypeScriptTarget;

/// Implements the `Target` trait for TypeScript code generation.
impl Target for TypeScriptTarget {
    /// Maps a Firebird type to a TypeScript type.
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "string".to_string(),
            FbType::BlobBinary | FbType::Blob => "string".to_string(), // base64
            FbType::Numeric { .. } | FbType::Decimal { .. } => "number".to_string(),
            FbType::Float | FbType::DoublePrecision => "number".to_string(),
            FbType::SmallInt | FbType::Integer | FbType::BigInt => "number".to_string(),
            FbType::Int128 => "string".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "string".to_string()
            }
            FbType::Cstring => "string".to_string(),
            FbType::Date | FbType::Time | FbType::Timestamp => "string".to_string(), // ISO format
            FbType::Boolean => "boolean".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "number".to_string(),
            FbType::Unknown(_) => "unknown".to_string(),
        }
    }

    /// Generates a TypeScript interface for the table.
    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let interface_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        if opts.include_comments {
            b.push_str(&format!(
                "// {} maps to table {}\n",
                interface_name, schema.table
            ));
        }

        b.push_str(&format!("export interface {} {{\n", interface_name));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let ts_type = self.type_for(&ft);
            let nullable = is_nullable(col);

            let comment = if opts.include_comments && opts.include_fks {
                fks.iter()
                    .find(|fk| fk.column == col.name)
                    .map(|fk| format!("    /** FK → {}.{} */\n", fk.ref_table, fk.ref_column))
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let opt = if nullable { "?" } else { "" };
            b.push_str(&comment);
            b.push_str(&format!("    {}{}: {};\n", field_name, opt, ts_type));
        }

        b.push_str("}");

        // Example query with aliases matching field names
        if opts.include_examples {
            b.push_str("\n\n// Example query (aliases must match field names):\n");
            let aliases: Vec<String> = schema
                .columns
                .iter()
                .map(|col| format!("{} AS {}", col.name, to_camel_case(&col.name)))
                .collect();
            b.push_str(&format!(
                "// SELECT {} FROM {}\n",
                aliases.join(", "),
                schema.table
            ));
            b.push_str(&format!(
                "// const rows: {}[] = await db.query(sql);\n",
                interface_name
            ));
            b.push_str(&format!(
                "// const rows: {}[] = await db.query(sql);\n",
                interface_name
            ));
        }

        vec![GeneratedFile::new(
            format!("{}.ts", interface_name),
            b,
            "typescript",
        )]
    }
}
