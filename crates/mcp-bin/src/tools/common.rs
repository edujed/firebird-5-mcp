//! Common utilities shared by all tool handlers.

use firebird_5_mcp_core::types::format_cell;
use firebird_5_mcp_core::{Config, FirebirdClient, query};
use serde_json::Value;

/// Extracts a string argument from the JSON args.
pub fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extracts a boolean argument from the JSON args.
pub fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

/// Opens a client for the requested database (or the default).
pub fn open_client(cfg: &Config, args: &Value) -> Result<FirebirdClient, String> {
    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())
}

/// Renders a query result as a Markdown table.
pub fn result_to_text(result: &query::QueryResult) -> String {
    let mut b = String::new();
    b.push_str(&format!(
        "### Query Results ({} rows)\n\n",
        result.rows.len()
    ));
    if result.columns.is_empty() {
        b.push_str("No columns returned.\n");
        return b;
    }
    let cols = result
        .columns
        .iter()
        .map(|c| format!("{} ({})", c.name, c.type_name))
        .collect::<Vec<_>>()
        .join(", ");
    b.push_str(&format!("Columns: {cols}\n\n"));
    b.push_str(
        &result
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(" | "),
    );
    b.push('\n');
    b.push_str(
        &result
            .columns
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | "),
    );
    b.push('\n');
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(|v| format_cell(v)).collect();
        b.push_str(&cells.join(" | "));
        b.push('\n');
    }
    b
}

/// Returns "" if n is 1, "s" otherwise.
pub fn plural_suffix(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Gets a cell value as text, or "-" if not present.
pub fn cell_text(row: &[query::Value], idx: usize) -> String {
    row.get(idx)
        .map(|v| format_cell(v))
        .unwrap_or_else(|| "-".to_string())
}

/// Gets a cell value as an integer, or 0 if not present.
pub fn cell_int(row: &[query::Value], idx: usize) -> i64 {
    match row.get(idx) {
        Some(query::Value::Int(n)) => *n,
        Some(query::Value::Text(s)) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

/// Gets a cell value as text, or a default if not present or empty.
pub fn cell_text_or(row: &[query::Value], idx: usize, default: &str) -> String {
    match row.get(idx) {
        Some(query::Value::Text(s)) if !s.is_empty() => s.clone(),
        _ => default.to_string(),
    }
}
