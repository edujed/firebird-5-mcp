//! Markdown formatting for table schemas.

use super::TableSchema;

/// Formats a table schema as Markdown (for use in describe_objects).
pub fn format_schema_markdown(schema: &TableSchema) -> String {
    use crate::types::format_column_type;
    let mut b = String::new();

    b.push_str("#### Columns\n\n");
    b.push_str("NAME | TYPE | NULL | DEFAULT | POSITION\n");
    b.push_str("--- | --- | --- | --- | ---\n");
    for col in &schema.columns {
        let type_str = format_column_type(
            col.type_num,
            col.sub_type,
            col.size,
            col.precision,
            col.scale,
        );
        // In Firebird, RDB$NULL_FLAG = 0 means the column CAN be NULL,
        // and RDB$NULL_FLAG = 1 means the column CANNOT be NULL.
        let null_str = if col.nullable.map(|n| n == 0).unwrap_or(true) {
            "YES"
        } else {
            "NO"
        };
        let default = col.default.as_deref().unwrap_or("-");
        let position = col
            .position
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        b.push_str(&format!(
            "{} | {} | {null_str} | {default} | {position}\n",
            col.name, type_str
        ));
    }

    if !schema.indexes.is_empty() {
        b.push('\n');
        b.push_str("#### Indexes\n\n");
        b.push_str("NAME | UNIQUE | TYPE | COLUMNS\n");
        b.push_str("--- | --- | --- | ---\n");
        for idx in &schema.indexes {
            let unique = if idx.unique { "YES" } else { "NO" };
            let type_str = idx
                .r#type
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string());
            b.push_str(&format!(
                "{} | {unique} | {type_str} | {}\n",
                idx.name, idx.columns
            ));
        }
    }

    if !schema.constraints.is_empty() {
        b.push('\n');
        b.push_str("#### Constraints\n\n");
        b.push_str("NAME | TYPE\n");
        b.push_str("--- | ---\n");
        for con in &schema.constraints {
            b.push_str(&format!("{} | {}\n", con.name, con.r#type));
        }
    }

    if !schema.triggers.is_empty() {
        b.push('\n');
        b.push_str("#### Triggers\n\n");
        b.push_str("NAME | TYPE | SEQUENCE\n");
        b.push_str("--- | --- | ---\n");
        for trg in &schema.triggers {
            let type_str = trg
                .r#type
                .map(|t| t.to_string())
                .unwrap_or_else(|| "-".to_string());
            let seq = trg
                .sequence
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string());
            b.push_str(&format!("{} | {type_str} | {seq}\n", trg.name));
        }
    }

    if !schema.comments.is_empty() {
        b.push('\n');
        b.push_str("#### Comments\n\n");
        for (key, value) in &schema.comments {
            b.push_str(&format!("- {key}: {value}\n"));
        }
    }

    b
}
