//! Rust (sqlx) target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::naming::{to_pascal_case, to_snake_case};
use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

pub struct RustTarget;

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

impl Target for RustTarget {
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "String".to_string(),
            FbType::BlobBinary | FbType::Blob => "Vec<u8>".to_string(),
            FbType::Numeric { .. } | FbType::Decimal { .. } => "Decimal".to_string(),
            FbType::Float | FbType::DoublePrecision => "f64".to_string(),
            FbType::SmallInt => "i16".to_string(),
            FbType::Integer | FbType::BigInt | FbType::Int128 => "i32".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "String".to_string()
            }
            FbType::Cstring => "String".to_string(),
            FbType::Date => "NaiveDate".to_string(),
            FbType::Time => "NaiveTime".to_string(),
            FbType::Timestamp => "NaiveDateTime".to_string(),
            FbType::Boolean => "bool".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "f64".to_string(),
            FbType::Unknown(_) => "i32".to_string(),
        }
    }

    fn dto_type_for(&self, t: &FbType) -> String {
        format!("Option<{}>", self.type_for(t))
    }

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], include_fks: bool) -> String {
        let struct_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str(&format!(
            "// {} maps to table {}\n",
            struct_name, schema.table
        ));
        b.push_str("#[derive(Debug, Clone, FromRow)]\n");
        b.push_str(&format!("pub struct {} {{\n", struct_name));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let base_type = self.type_for(&ft);
            let nullable = is_nullable(col);

            let mut comment = format!("    /// Column: {}", col.name);
            if include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    comment.push_str(&format!(" — FK → {}.{}", fk.ref_table, fk.ref_column));
                }
            }
            comment.push('\n');

            let type_str = if nullable {
                format!("Option<{}>", base_type)
            } else {
                base_type
            };

            b.push_str(&comment);
            b.push_str(&format!("    pub {}: {},\n", field_name, type_str));
        }

        b.push_str("}\n");

        // Example query with aliases matching field names
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
        b
    }
}
