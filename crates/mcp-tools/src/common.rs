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

/// Percent-encodes a single path segment for use in a URL query string.
///
/// Only the characters that are safe in a URL path/query are kept as-is; every
/// other byte is encoded as `%XX` (UTF-8). This is a minimal, dependency-free
/// encoder — enough for file paths, which are typically simple.
fn percent_encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Builds the HTTP download URL for a file that lives inside the storage
/// directory, when the transport exposes a `/files` endpoint.
///
/// Returns `None` when:
/// - `base_url` is `None` (stdio mode — no HTTP endpoint), or
/// - `abs_path` is not inside `storage_base` (e.g. an export written to a
///   caller-supplied path outside the storage dir, which `/files` cannot serve).
///
/// Otherwise returns `{base_url}/files/<url-encoded relative path>`.
pub fn file_download_url(
    base_url: Option<&str>,
    abs_path: &str,
    storage_base: &str,
) -> Option<String> {
    let base = base_url?;
    let abs = std::path::Path::new(abs_path);
    let rel = abs.strip_prefix(storage_base).ok()?;
    let encoded = rel
        .components()
        .map(|c| percent_encode_segment(c.as_os_str().to_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("/");
    if encoded.is_empty() {
        return None;
    }
    // The `/files` endpoint takes the relative path as a `path` query
    // parameter (see mcp-stdio-bin/src/files.rs), so the URL is
    // `{base}/files?path=<encoded>`.
    Some(format!("{base}/files?path={encoded}"))
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
    files_base_url: Option<&str>,
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

    // Build structured content
    let mut file_entries: Vec<Value> = Vec::with_capacity(result.files.len());
    let mut download_urls: Vec<String> = Vec::new();
    for f in &result.files {
        let mut entry = json!({
            "name": f.name,
            "path": f.path,
            "size": f.size,
            "lines": f.lines,
            "format": f.format,
        });
        if let Some(url) = file_download_url(files_base_url, &f.path, &base_dir) {
            entry["download_url"] = json!(url);
            download_urls.push(url);
        }
        file_entries.push(entry);
    }

    let mut text = storage::summary(&result, &previews);
    if !download_urls.is_empty() {
        text.push_str("\n\nDownload URLs:\n");
        for url in &download_urls {
            text.push_str(&format!("- {url}\n"));
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_download_url_none_base() {
        assert_eq!(
            file_download_url(
                None,
                "/home/u/.firebird-mcp/output/db_pq/x.parquet",
                "/home/u/.firebird-mcp/output"
            ),
            None
        );
    }

    #[test]
    fn test_file_download_url_outside_storage() {
        assert_eq!(
            file_download_url(
                Some("http://10.0.0.5:8080"),
                "/elsewhere/x.parquet",
                "/home/u/.firebird-mcp/output"
            ),
            None
        );
    }

    #[test]
    fn test_file_download_url_inside_storage() {
        let url = file_download_url(
            Some("http://10.0.0.5:8080"),
            "/home/u/.firebird-mcp/output/teste_parquet/20260101/data.parquet",
            "/home/u/.firebird-mcp/output",
        )
        .unwrap();
        assert_eq!(
            url,
            "http://10.0.0.5:8080/files?path=teste_parquet/20260101/data.parquet"
        );
    }

    #[test]
    fn test_file_download_url_encodes_spaces() {
        let url = file_download_url(
            Some("http://10.0.0.5:8080"),
            "/base/out/my file.csv",
            "/base",
        )
        .unwrap();
        assert_eq!(url, "http://10.0.0.5:8080/files?path=out/my%20file.csv");
    }

    #[test]
    fn test_file_download_url_base_with_trailing_slash() {
        // The base URL is normalised (trailing slash stripped) by the caller
        // (main.rs) before being stored, so a well-formed base has no trailing
        // slash. This documents the expected input contract.
        let url = file_download_url(Some("http://10.0.0.5:8080"), "/base/out/x.parquet", "/base")
            .unwrap();
        assert_eq!(url, "http://10.0.0.5:8080/files?path=out/x.parquet");
    }
}
