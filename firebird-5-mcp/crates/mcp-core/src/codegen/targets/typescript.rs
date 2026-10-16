//! TypeScript target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

pub struct TypeScriptTarget;

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

impl Target for TypeScriptTarget {
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "string".to_string(),
            FbType::BlobBinary | FbType::Blob => "string".to_string(), // base64
            FbType::Numeric { .. } | FbType::Decimal { .. } => "number".to_string(),
            FbType::Float | FbType::DoublePrecision => "number".to_string(),
            FbType::SmallInt | FbType::Integer | FbType::BigInt | FbType::Int128 => {
                "number".to_string()
            }
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

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], include_fks: bool) -> String {
        let interface_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str(&format!("export interface {} {{\n", interface_name));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let ts_type = self.type_for(&ft);
            let nullable = is_nullable(col);

            let mut comment = String::new();
            if include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    comment = format!("    /** FK → {}.{} */\n", fk.ref_table, fk.ref_column);
                }
            }

            let opt = if nullable { "?" } else { "" };
            b.push_str(&comment);
            b.push_str(&format!("    {}{}: {};\n", field_name, opt, ts_type));
        }

        b.push_str("}");
        b
    }
}
