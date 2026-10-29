//! Protobuf (Connect-RPC / gRPC) target.

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_pascal_case, to_snake_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

/// Protobuf code generation target (Connect-RPC / gRPC).
pub struct ProtobufTarget;

/// Implements the `Target` trait for Protobuf code generation.
impl Target for ProtobufTarget {
    /// Maps a Firebird type to a Protobuf scalar type.
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "string".to_string(),
            FbType::BlobBinary | FbType::Blob => "bytes".to_string(),
            FbType::Numeric { .. } | FbType::Decimal { .. } => "string".to_string(), // precision
            FbType::Float | FbType::DoublePrecision => "double".to_string(),
            FbType::SmallInt | FbType::Integer => "int32".to_string(),
            FbType::BigInt => "int64".to_string(),
            FbType::Int128 => "string".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "string".to_string()
            }
            FbType::Cstring => "string".to_string(),
            FbType::Date | FbType::Time | FbType::Timestamp => "string".to_string(), // ISO format
            FbType::Boolean => "bool".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "double".to_string(),
            FbType::Unknown(_) => "string".to_string(),
        }
    }

    /// Generates a Protobuf message definition for the table.
    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let message_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        b.push_str("syntax = \"proto3\";\n\n");
        // Package: use opts.package if provided, else derive from table name
        let pkg = opts
            .package
            .clone()
            .unwrap_or_else(|| schema.table.to_lowercase());
        b.push_str(&format!("package {};\n\n", pkg));

        // Nullable columns comment
        if opts.include_comments {
            let nullable_cols: Vec<&str> = schema
                .columns
                .iter()
                .filter(|c| is_nullable(c))
                .map(|c| c.name.as_str())
                .collect();
            if !nullable_cols.is_empty() {
                b.push_str(&format!(
                    "// Nullable columns: {}\n",
                    nullable_cols.join(", ")
                ));
            }
        }

        b.push_str(&format!("message {} {{\n", message_name));

        for (idx, col) in schema.columns.iter().enumerate() {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let proto_type = self.type_for(&ft);
            let field_num = idx + 1; // Protobuf fields start at 1

            if opts.include_comments {
                let mut comments = Vec::new();
                if let Some(comment) = schema.comments.get(&col.name) {
                    comments.push(comment.clone());
                }
                if opts.include_fks {
                    if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                        comments.push(format!("FK → {}.{}", fk.ref_table, fk.ref_column));
                    }
                }
                for c in &comments {
                    b.push_str(&format!("    // {}\n", c));
                }
            }

            b.push_str(&format!(
                "    {} {} = {};\n",
                proto_type, field_name, field_num
            ));
        }

        b.push_str("}");

        vec![GeneratedFile::new(
            format!("{}.proto", schema.table.to_lowercase()),
            b,
            "protobuf",
        )]
    }
}
