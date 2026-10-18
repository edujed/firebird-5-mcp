//! Common utilities shared by all tool handlers.

use firebird_5_mcp_core::codegen::targets::GeneratedFile;
use firebird_5_mcp_core::storage;
use firebird_5_mcp_core::types::format_cell;
use firebird_5_mcp_core::{Config, FirebirdClient, mlog, query};
use serde_json::{Value, json};

use super::ToolResult;

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

/// Extracts a non-negative integer argument, accepting either a JSON number
/// or a numeric string. LLMs frequently send numbers where the schema says
/// string (and vice-versa), so we accept both to avoid spurious failures.
pub fn arg_u32(args: &Value, key: &str) -> Option<u32> {
    match args.get(key)? {
        Value::Number(n) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
        Value::String(s) => s.trim().parse::<u32>().ok(),
        _ => None,
    }
}

/// Opens a client for the requested database (or the default).
pub fn open_client(cfg: &Config, args: &Value) -> Result<FirebirdClient, String> {
    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    mlog!("connecting to '{}' at {}", db_name, db.path);
    FirebirdClient::new(&cfg.server, &db.path).map_err(|e| {
        mlog!("connection to '{}' FAILED: {e}", db_name);
        e.to_string()
    })
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

/// Resolves the storage base directory from config or the `outputDirPath` argument.
///
/// Priority: `outputDirPath` arg > `config.storage.dir` > `~/.firebird-mcp/output/`.
/// Always returns `Some` (the default dir is a safe fallback).
pub fn storage_dir(cfg: &Config, args: &Value) -> Option<String> {
    if let Some(output_dir) = arg_str(args, "outputDirPath").filter(|s| !s.is_empty()) {
        return Some(output_dir);
    }
    if let Some(storage) = &cfg.storage {
        if let Some(dir) = storage.effective_dir() {
            return dir.to_str().map(|s| s.to_string());
        }
    }
    // Fallback: ~/.firebird-mcp/output/
    firebird_5_mcp_core::storage::default_storage_dir()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
}

/// Writes generated files to disk and returns a `ToolResult` with a summary
/// and structured content.
///
/// The output directory is resolved from (in priority order):
/// 1. `outputDirPath` argument (per-call override)
/// 2. `config.storage.dir` (configured default)
/// 3. `~/.firebird-mcp/output/` (safe fallback)
pub fn save_or_inline(
    cfg: &Config,
    args: &Value,
    db_name: &str,
    format: &str,
    files: &[GeneratedFile],
) -> Result<ToolResult, String> {
    let base_dir =
        storage_dir(cfg, args).ok_or_else(|| "Could not resolve storage directory".to_string())?;

    let result = storage::write_files(&base_dir, db_name, format, files)
        .map_err(|e| format!("Failed to save files: {e}"))?;

    // Generate previews (first 10 + last 5 lines, max 120 chars per line)
    let mut previews = std::collections::HashMap::new();
    for file in files {
        previews.insert(
            file.name.clone(),
            storage::preview(&file.content, 10, 5, 120),
        );
    }

    let text = storage::summary(&result, &previews);

    // Build structured content
    let file_entries: Vec<Value> = result
        .files
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "path": f.path,
                "size": f.size,
                "lines": f.lines,
                "format": f.format,
            })
        })
        .collect();

    let preview_entries: Vec<Value> = previews
        .iter()
        .map(|(name, pv)| json!({ "name": name, "preview": pv }))
        .collect();

    let structured = json!({
        "output_dir": result.output_dir,
        "database": db_name,
        "format": format,
        "files": file_entries,
        "previews": preview_entries,
    });

    Ok(ToolResult::with_structured(text, structured))
}
