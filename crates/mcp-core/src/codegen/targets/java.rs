//! Java (JPA + Lombok) target.

use super::Target;
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
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

    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
        let class_name = to_pascal_case(&schema.table);
        let mut b = String::new();

        // Package declaration
        if let Some(pkg) = &opts.package {
            b.push_str(&format!("package {};\n\n", pkg));
        }

        // Imports
        for imp in java_imports(schema, opts.is_view) {
            b.push_str(&format!("import {}\n", imp));
        }
        b.push('\n');

        // Class annotations
        if !opts.is_view {
            b.push_str("@Entity\n");
            b.push_str(&format!("@Table(name = \"{}\")\n", schema.table));
        }
        b.push_str("@Data\n");
        b.push_str("@NoArgsConstructor\n");
        b.push_str("@AllArgsConstructor\n");
        b.push_str("@Builder\n");
        b.push_str(&format!("public class {} {{\n\n", class_name));

        // PK columns: prefer PRIMARY KEY constraint, fall back to first unique index
        let pk_cols: Vec<String> = if let Some(pk_constraint) = schema
            .constraints
            .iter()
            .find(|c| c.r#type.eq_ignore_ascii_case("PRIMARY KEY"))
        {
            // Find the index matching the PK constraint name
            schema
                .indexes
                .iter()
                .find(|i| i.name == pk_constraint.name)
                .map(|i| i.columns.split(',').map(|c| c.trim().to_string()).collect())
                .unwrap_or_default()
        } else {
            // Fallback: first unique index
            schema
                .indexes
                .iter()
                .find(|i| i.unique)
                .map(|i| i.columns.split(',').map(|c| c.trim().to_string()).collect())
                .unwrap_or_default()
        };

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_camel_case(&col.name);
            let java_type = self.type_for(&ft);

            // Check if this column is a FK and include_fks is on
            if opts.include_fks {
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
            if !opts.is_view && pk_cols.contains(&col.name) {
                b.push_str("    @Id\n");
                b.push_str("    @GeneratedValue(strategy = GenerationType.SEQUENCE)\n");
            }
            if ft.is_blob() {
                b.push_str("    @Lob\n");
            }
            if ft.is_char() && ft.char_size() > 0 {
                b.push_str(&format!(
                    "    @Column(name = \"{}\", length = {})\n",
                    col.name,
                    ft.char_size()
                ));
            } else if let Some((precision, scale)) = ft.precision_scale() {
                let scale_abs = scale.abs();
                if precision > 0 && scale_abs > 0 {
                    b.push_str(&format!(
                        "    @Column(name = \"{}\", precision = {}, scale = {})\n",
                        col.name, precision, scale_abs
                    ));
                } else if precision > 0 {
                    b.push_str(&format!(
                        "    @Column(name = \"{}\", precision = {})\n",
                        col.name, precision
                    ));
                } else {
                    b.push_str(&format!("    @Column(name = \"{}\")\n", col.name));
                }
            } else {
                b.push_str(&format!("    @Column(name = \"{}\")\n", col.name));
            }
            b.push_str(&format!("    private {} {};\n\n", java_type, field_name));
        }

        b.push_str("}");

        // Example query with aliases matching field names
        if opts.include_examples {
            b.push_str("\n\n// Example query (aliases must match field names for JPA):\n");
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
            b.push_str(&format!("// List<{}> rows = em.createQuery(\"SELECT {} FROM {}\", {}.class).getResultList();\n", class_name, class_name, class_name, class_name));
        }

        b
    }
}

fn java_imports(schema: &TableSchema, is_view: bool) -> Vec<&'static str> {
    let mut imports: Vec<&'static str> = Vec::new();
    if !is_view {
        imports.push("jakarta.persistence.*");
    }
    imports.push("lombok.*");
    // Check if any column is a PK (needs GenerationType) — only for entities
    if !is_view {
        let has_pk = schema
            .constraints
            .iter()
            .any(|c| c.r#type.eq_ignore_ascii_case("PRIMARY KEY"))
            || schema.indexes.iter().any(|i| i.unique);
        if has_pk {
            imports.push("jakarta.persistence.GenerationType");
        }
    }
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
