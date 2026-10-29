//! File-save helper: writes generated files to disk and returns a
//! `ToolResult` with a summary and structured content.
//!
//! The output directory is resolved from (in priority order):
//! 1. `outputDirPath` argument (per-call override)
//! 2. `config.storage_dir` (configured default)
//! 3. `~/.firebird-mcp/output/` (safe fallback)

use firebird_5_mcp_core::codegen::targets::GeneratedFile;
use firebird_5_mcp_core::storage;
use mcp_core::Config;
use mcp_protocol::common::{file_download_url, storage_dir};
use mcp_types::ToolResult;
use serde_json::{Value, json};

/// Writes generated files to disk and returns a `ToolResult` with a summary
/// and structured content.
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
