//! Go (sqlx) target.

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

pub struct GoTarget;

impl Target for GoTarget {
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "string".to_string(),
            FbType::BlobBinary | FbType::Blob => "[]byte".to_string(),
            FbType::Numeric { .. } | FbType::Decimal { .. } => "decimal.Decimal".to_string(),
            FbType::Float | FbType::DoublePrecision => "float64".to_string(),
            FbType::SmallInt => "int16".to_string(),
            FbType::Integer => "int32".to_string(),
            FbType::BigInt => "int64".to_string(),
            FbType::Int128 => "string".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "string".to_string()
            }
            FbType::Cstring => "string".to_string(),
            FbType::Date | FbType::Time | FbType::Timestamp => "time.Time".to_string(),
            FbType::Boolean => "bool".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "decimal.Decimal".to_string(),
            FbType::Unknown(_) => "interface{}".to_string(),
        }
    }

    fn dto_type_for(&self, t: &FbType) -> String {
        go_null_type(&self.type_for(t))
    }

    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let struct_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        // Package declaration
        if let Some(pkg) = &opts.package {
            b.push_str(&format!("package {}\n\n", pkg));
        }

        if opts.include_comments {
            b.push_str(&format!(
                "// {} maps to table {}\n",
                struct_name, schema.table
            ));
        }

        // Emit required imports
        let imports = go_imports(schema);
        if !imports.is_empty() {
            b.push_str("import (\n");
            for imp in &imports {
                b.push_str(&format!("\t\"{}\"\n", imp));
            }
            b.push_str(")\n\n");
        }

        b.push_str(&format!("type {} struct {{\n", struct_name));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_pascal_case(&col.name);
            let base_type = self.type_for(&ft);
            let nullable = is_nullable(col);
            let json_name = to_camel_case(&col.name);

            let type_str = if nullable {
                go_null_type(&base_type)
            } else {
                base_type
            };

            let comment = if opts.include_comments && opts.include_fks {
                fks.iter()
                    .find(|fk| fk.column == col.name)
                    .map(|fk| format!(" // FK → {}.{}", fk.ref_table, fk.ref_column))
                    .unwrap_or_default()
            } else {
                String::new()
            };

            b.push_str(&format!(
                "    {:<20} {:<20} `db:\"{}\" json:\"{}\"`{}\n",
                field_name, type_str, col.name, json_name, comment
            ));
        }

        b.push_str("}");

        // Example query with aliases matching field names
        if opts.include_examples {
            b.push_str("\n\n// Example query (aliases must match db tags for sqlx):\n");
            let aliases: Vec<String> = schema
                .columns
                .iter()
                .map(|col| format!("{} AS {}", col.name, to_pascal_case(&col.name)))
                .collect();
            b.push_str(&format!(
                "// SELECT {} FROM {}\n",
                aliases.join(", "),
                schema.table
            ));
            b.push_str(&format!(
                "// var rows []{}; db.Select(&rows, query)\n",
                struct_name
            ));
            b.push_str(&format!(
                "// var rows []{}; db.Select(&rows, query)\n",
                struct_name
            ));
        }

        vec![GeneratedFile::new(
            format!("{}.go", struct_name.to_lowercase()),
            b,
            "go",
        )]
    }
}

fn go_null_type(t: &str) -> String {
    format!("*{t}")
}

/// Returns the import paths needed for the types used in this table.
fn go_imports(schema: &TableSchema) -> Vec<&'static str> {
    let mut imports: Vec<&'static str> = Vec::new();

    for col in &schema.columns {
        let ft = FbType::from_column(col);
        match ft {
            FbType::Date | FbType::Time | FbType::Timestamp => imports.push("time"),
            FbType::Numeric { .. }
            | FbType::Decimal { .. }
            | FbType::DecFloat16
            | FbType::DecFloat34 => imports.push("github.com/shopspring/decimal"),
            _ => {}
        }
    }

    imports.sort();
    imports.dedup();
    imports
}
