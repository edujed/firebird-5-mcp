//! Go (sqlx) target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

pub struct GoTarget;

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

impl Target for GoTarget {
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
            FbType::Unknown(_) => "interface{}".to_string(),
        }
    }

    fn dto_type_for(&self, t: &FbType) -> String {
        go_null_type(&self.type_for(t))
    }

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], include_fks: bool) -> String {
        let struct_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str(&format!(
            "// {} maps to table {}\n",
            struct_name, schema.table
        ));
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

            let mut comment = String::new();
            if include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    comment = format!(" // FK → {}.{}", fk.ref_table, fk.ref_column);
                }
            }

            b.push_str(&format!(
                "    {:<20} {:<20} `db:\"{}\" json:\"{}\"`{}\n",
                field_name, type_str, col.name, json_name, comment
            ));
        }

        b.push_str("}");
        b
    }
}

fn go_null_type(t: &str) -> String {
    match t {
        "i16" => "sql.NullInt16".to_string(),
        "i32" => "sql.NullInt32".to_string(),
        "f64" => "sql.NullFloat64".to_string(),
        "String" => "sql.NullString".to_string(),
        "bool" => "sql.NullBool".to_string(),
        "NaiveDate" => "*time.Time".to_string(),
        "NaiveTime" => "*time.Time".to_string(),
        "NaiveDateTime" => "*time.Time".to_string(),
        "Decimal" => "*Decimal".to_string(),
        _ => format!("*{t}"),
    }
}
