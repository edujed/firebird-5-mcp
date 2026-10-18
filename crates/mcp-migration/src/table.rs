//! Table operations: existence check, creation, and append verification.
//!
//! Centralises the RDB$ introspection queries that were previously
//! duplicated across `import.rs` and `csv.rs`.

use arrow::datatypes::Schema;
use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::query::{Value, exec, query_with_param};

use crate::types::{arrow_to_firebird, check_append_compatibility};

/// Returns `true` if the table exists in the database.
pub fn table_exists(client: &mut FirebirdClient, table: &str) -> Result<bool, String> {
    let sql = "SELECT COUNT(*) FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = ?";
    let result = query_with_param(client, sql, table).map_err(|e| e.to_string())?;
    let count = result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| match v {
            Value::Int(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    Ok(count > 0)
}

/// Creates a table from an Arrow schema. Fails if the table already exists.
///
/// Column types are derived via [`arrow_to_firebird`].
pub fn create_table_from_arrow(
    client: &mut FirebirdClient,
    table: &str,
    schema: &Schema,
) -> Result<(), String> {
    if table_exists(client, table)? {
        return Err(format!(
            "table '{table}' already exists (use mode 'append' to add data)"
        ));
    }

    let col_defs: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| {
            let fb_type = arrow_to_firebird(f.data_type());
            format!("\"{}\" {}", f.name(), fb_type)
        })
        .collect();
    let ddl = format!("CREATE TABLE \"{}\" ({})", table, col_defs.join(", "));
    exec(client, &ddl).map_err(|e| format!("CREATE TABLE failed: {e}"))?;
    Ok(())
}

/// Creates a table from explicit column names and DDL type strings.
///
/// Used by the CSV import path where types are inferred from sample data.
/// Columns with any empty values in the sample are nullable; otherwise
/// `NOT NULL` is added.
pub fn create_table_from_types(
    client: &mut FirebirdClient,
    table: &str,
    columns: &[String],
    types: &[String],
    sample_rows: &[Vec<String>],
) -> Result<(), String> {
    if table_exists(client, table)? {
        return Err(format!(
            "table '{table}' already exists (use mode 'append' to add data)"
        ));
    }

    let col_defs: Vec<String> = columns
        .iter()
        .enumerate()
        .zip(types.iter())
        .map(|((col_idx, name), ty)| {
            let has_empty = sample_rows.iter().any(|row| row[col_idx].is_empty());
            if has_empty {
                format!("\"{name}\" {ty}")
            } else {
                format!("\"{name}\" {ty} NOT NULL")
            }
        })
        .collect();
    let ddl = format!("CREATE TABLE \"{}\" ({})", table, col_defs.join(", "));
    exec(client, &ddl).map_err(|e| format!("CREATE TABLE failed: {e}"))?;
    Ok(())
}

/// Verifies that the table exists and its structure is compatible with the
/// given Arrow schema for `append` mode.
pub fn verify_append(
    client: &mut FirebirdClient,
    table: &str,
    schema: &Schema,
) -> Result<(), String> {
    let sql = r#"
        SELECT RF.RDB$FIELD_NAME, F.RDB$FIELD_TYPE
        FROM RDB$RELATION_FIELDS RF
        JOIN RDB$FIELDS F ON F.RDB$FIELD_NAME = RF.RDB$FIELD_SOURCE
        WHERE RF.RDB$RELATION_NAME = ?
        ORDER BY RF.RDB$FIELD_POSITION
    "#;
    let result = query_with_param(client, sql, table).map_err(|e| e.to_string())?;

    if result.rows.is_empty() {
        return Err(format!(
            "table '{table}' not found (use mode 'create' to create it)"
        ));
    }

    let fb_columns: Vec<(String, i64)> = result
        .rows
        .iter()
        .map(|row| {
            let name = row
                .first()
                .and_then(|v| match v {
                    Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let type_num = row
                .get(1)
                .and_then(|v| match v {
                    Value::Int(n) => Some(*n),
                    _ => None,
                })
                .unwrap_or(0);
            (name, type_num)
        })
        .collect();

    check_append_compatibility(schema, &fb_columns)?;
    Ok(())
}
