//! Java (JPA + Lombok) target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::naming::{to_camel_case, to_pascal_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

pub struct JavaTarget;

impl Target for JavaTarget {
    fn type_for(&self, t: &FbType) -> String {
        match t {
            FbType::BlobText => "String".to_string(),
            FbType::BlobBinary | FbType::Blob => "byte[]".to_string(),
            FbType::Numeric { .. } | FbType::Decimal { .. } => "BigDecimal".to_string(),
            FbType::Float | FbType::DoublePrecision => "Double".to_string(),
            FbType::SmallInt => "Short".to_string(),
            FbType::Integer | FbType::BigInt | FbType::Int128 => "Integer".to_string(),
            FbType::Char { .. } | FbType::Varchar { .. } | FbType::Nvarchar { .. } => {
                "String".to_string()
            }
            FbType::Cstring => "String".to_string(),
            FbType::Date => "LocalDate".to_string(),
            FbType::Time => "LocalTime".to_string(),
            FbType::Timestamp => "LocalDateTime".to_string(),
            FbType::Boolean => "Boolean".to_string(),
            FbType::DecFloat16 | FbType::DecFloat34 => "BigDecimal".to_string(),
            FbType::Unknown(_) => "Object".to_string(),
        }
    }

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], include_fks: bool) -> String {
        let class_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        // Imports
        for imp in java_imports(schema) {
            b.push_str(&format!("import {}\n", imp));
        }
        b.push('\n');

        // Class annotations
        b.push_str("@Entity\n");
        b.push_str(&format!("@Table(name = \"{}\")\n", schema.table));
        b.push_str("@Data\n");
        b.push_str("@NoArgsConstructor\n");
        b.push_str("@AllArgsConstructor\n");
        b.push_str("@Builder\n");
        b.push_str(&format!("public class {} {{\n\n", class_name));

        // PK columns
        let pk_cols: Vec<String> = schema
            .indexes
            .iter()
            .filter(|i| i.unique && i.name.starts_with("PK_"))
            .flat_map(|i| i.columns.split(',').map(|c| c.trim().to_string()))
            .collect();

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let java_type = self.type_for(&ft);

            // Check if this column is a FK and include_fks is on
            if include_fks {
                if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                    let entity_name = to_pascal_case(&fk.ref_table);
                    b.push_str("    @ManyToOne\n");
                    b.push_str(&format!("    @JoinColumn(name = \"{}\")\n", col.name));
                    b.push_str(&format!(
                        "    private {} {};\n\n",
                        entity_name,
                        to_camel_case(&fk.column)
                    ));
                    continue;
                }
            }

            // Regular column
            if pk_cols.contains(&col.name) {
                b.push_str("    @Id\n");
            }
            if ft.is_blob() {
                b.push_str("    @Lob\n");
            }
            if ft.is_char() && ft.char_size() > 0 {
                let char_size = if ft.is_nvarchar() {
                    ft.char_size() / 2
                } else {
                    ft.char_size() / 4
                };
                b.push_str(&format!(
                    "    @Column(name = \"{}\", length = {})",
                    col.name, char_size
                ));
            } else if let Some((precision, scale)) = ft.precision_scale() {
                let scale_abs = scale.abs();
                if precision > 0 && scale_abs > 0 {
                    b.push_str(&format!(
                        "    @Column(name = \"{}\", precision = {}, scale = {})",
                        col.name, precision, scale_abs
                    ));
                } else if precision > 0 {
                    b.push_str(&format!(
                        "    @Column(name = \"{}\", precision = {})",
                        col.name, precision
                    ));
                } else {
                    b.push_str(&format!("    @Column(name = \"{}\")", col.name));
                }
            } else {
                b.push_str(&format!("    @Column(name = \"{}\")", col.name));
            }
            b.push_str("\n");
            b.push_str(&format!("    private {} {};\n\n", java_type, field_name));
        }

        b.push_str("}");
        b
    }
}

fn java_imports(schema: &TableSchema) -> Vec<&'static str> {
    let mut imports: Vec<&'static str> = Vec::new();
    imports.push("jakarta.persistence.*");
    imports.push("lombok.*");
    for col in &schema.columns {
        let ft = FbType::from_column(col);
        if ft.is_numeric() || ft.is_decimal() {
            imports.push("java.math.BigDecimal");
        }
        if ft.is_date() {
            imports.push("java.time.LocalDate");
        }
        if ft.is_time() {
            imports.push("java.time.LocalTime");
        }
        if ft.is_timestamp() {
            imports.push("java.time.LocalDateTime");
        }
    }
    imports.sort();
    imports.dedup();
    imports
}
