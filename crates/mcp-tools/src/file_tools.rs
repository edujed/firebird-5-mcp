//! Handler for the `firebird_list_files` tool.
//!
//! Lists generated files (codegen, export, metadata) in the storage
//! directory. Works in both stdio and SSE transports — in stdio the client
//! reads files from the local filesystem; in SSE the client downloads them
//! via the `/files` HTTP endpoint.

use crate::ToolResult;
use crate::common::{arg_str, file_download_url, storage_dir};
use firebird_5_mcp_core::Config;
use serde_json::{Value, json};
use std::path::Path;

/// `firebird_list_files` — lists generated files in the storage directory.
///
/// Args (all optional):
/// - `database` — filter by database name (matches the `<db_snake>` prefix
///   of the subdirectory).
/// - `format` — filter by format (matches the `_<format>` suffix of the
///   subdirectory).
/// - `max_age_hours` — only include files modified within the last N hours.
///
/// Returns a Markdown table (text) and structured content with the file
/// list, total count, and total size.
pub fn handle_list_files(
    cfg: &Config,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    let base_dir =
        storage_dir(cfg, args).ok_or_else(|| "Could not resolve storage directory".to_string())?;

    let db_filter = arg_str(args, "database");
    let format_filter = arg_str(args, "format");
    let max_age_hours = args
        .get("max_age_hours")
        .and_then(|v| v.as_u64())
        .or_else(|| arg_str(args, "max_age_hours").and_then(|s| s.parse().ok()));

    let base = Path::new(&base_dir);
    if !base.exists() {
        return Ok(ToolResult::with_structured(
            format!("No files found. Storage directory `{base_dir}` does not exist yet."),
            json!({
                "output_dir": base_dir,
                "files": [],
                "total": 0,
                "total_size": "0 B"
            }),
        ));
    }

    let now = std::time::SystemTime::now();
    let mut files: Vec<Value> = Vec::new();
    let mut total_size: u64 = 0;

    // Walk the base directory. The structure is:
    //   <base>/<db_snake>_<format>/[<session>/]<file>
    // We collect all regular files, recording their relative path, size,
    // and modification time.
    let mut subdirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            }
        }
    }

    for subdir in &subdirs {
        let subdir_name = subdir.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Apply database filter: the subdirectory name starts with
        // `<db_snake>_`.
        if let Some(ref db) = db_filter {
            let db_snake = to_snake_case(db);
            if !subdir_name.starts_with(&format!("{db_snake}_")) {
                continue;
            }
        }

        // Apply format filter: the subdirectory name ends with `_<format>`.
        // The format is the last segment after the final underscore, but
        // only if there's no session subdirectory. We check the subdirectory
        // name directly (without session).
        if let Some(ref fmt) = format_filter {
            // The subdirectory is `<db_snake>_<format>` or
            // `<db_snake>_<format>_<session>`. We match if the name contains
            // `_<format>` as a segment.
            let segments: Vec<&str> = subdir_name.split('_').collect();
            if !segments.contains(&fmt.as_str()) {
                continue;
            }
        }

        // Recursively walk the subdirectory to find all files.
        collect_files(
            subdir,
            base,
            &mut files,
            &now,
            max_age_hours,
            &mut total_size,
            files_base_url,
        );
    }

    // Sort by modification time (newest first).
    files.sort_by(|a, b| {
        let a_time = a["modified"].as_str().unwrap_or("");
        let b_time = b["modified"].as_str().unwrap_or("");
        b_time.cmp(a_time)
    });

    // Build the Markdown text.
    let mut text = String::new();
    if files.is_empty() {
        text.push_str(&format!(
            "No files found in `{base_dir}`.\n\n\
             Use `firebird_generate_dev`, `firebird_export`, or \
             `firebird_metadata_extract` (with `outputDirPath`) to generate files."
        ));
    } else {
        text.push_str(&format!(
            "### Generated Files ({} files, {})\n\n",
            files.len(),
            format_size(total_size)
        ));
        text.push_str("| File | Size | Lines | Modified |\n");
        text.push_str("|------|------|-------|----------|\n");
        for f in &files {
            let name = f["name"].as_str().unwrap_or("-");
            let size = format_size(f["size"].as_u64().unwrap_or(0));
            let lines = f["lines"].as_u64().unwrap_or(0);
            let modified = f["modified"].as_str().unwrap_or("-");
            text.push_str(&format!("| {name} | {size} | {lines} | {modified} |\n"));
        }
    }

    let structured = json!({
        "output_dir": base_dir,
        "files": files,
        "total": files.len(),
        "total_size": format_size(total_size)
    });

    Ok(ToolResult::with_structured(text, structured))
}

/// Recursively collects files from a directory, applying the max_age filter.
fn collect_files(
    dir: &Path,
    base: &Path,
    files: &mut Vec<Value>,
    now: &std::time::SystemTime,
    max_age_hours: Option<u64>,
    total_size: &mut u64,
    files_base_url: Option<&str>,
) {
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        entries.extend(rd.flatten());
    }
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(
                &path,
                base,
                files,
                now,
                max_age_hours,
                total_size,
                files_base_url,
            );
        } else if path.is_file() {
            // Get file metadata.
            let metadata = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Apply max_age filter.
            if let Some(hours) = max_age_hours {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = now.duration_since(modified) {
                        if elapsed.as_secs() > hours * 3600 {
                            continue;
                        }
                    }
                }
            }

            // Get the relative path from the base directory.
            let rel_path = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            // Count lines (only for text files — skip if the file is too
            // large to avoid slow I/O).
            let size = metadata.len();
            let lines = if size < 10 * 1024 * 1024 {
                // 10 MB cap
                std::fs::read_to_string(&path)
                    .map(|content| content.lines().count() as u64)
                    .unwrap_or(0)
            } else {
                0
            };

            // Format the modification time as ISO 8601.
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| {
                    // Convert SystemTime to a human-readable string.
                    // We use a simple approach: seconds since epoch.
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                })
                .unwrap_or_else(|| "-".to_string());

            *total_size += size;
            let abs_path = path.to_string_lossy().to_string();
            let mut entry = json!({
                "name": rel_path,
                "path": abs_path,
                "size": size,
                "lines": lines,
                "modified": modified
            });
            if let Some(url) =
                file_download_url(files_base_url, &abs_path, base.to_str().unwrap_or(""))
            {
                entry["download_url"] = json!(url);
            }
            files.push(entry);
        }
    }
}

/// Converts a string to snake_case (for directory name matching).
///
/// Mirrors the logic in `mcp-core::storage::to_snake_case` so that the
/// database filter matches the subdirectory naming convention.
fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_was_lower_or_digit = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if prev_was_lower_or_digit {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_was_lower_or_digit = false;
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            prev_was_lower_or_digit = true;
        } else if c == '_' {
            out.push(c);
            prev_was_lower_or_digit = false;
        } else if c == ' ' || c == '-' || c == '.' {
            out.push('_');
            prev_was_lower_or_digit = false;
        }
    }
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_underscore = false;
    for c in out.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }
    collapsed.trim_matches('_').to_string()
}

/// Formats a byte size for display (e.g. "2.3 KB", "1.1 MB").
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("prod"), "prod");
        assert_eq!(to_snake_case("ProdDB"), "prod_db");
        assert_eq!(to_snake_case("prod-db"), "prod_db");
        assert_eq!(to_snake_case("MyProd.DB"), "my_prod_db");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
        assert_eq!(format_size(2_000_000_000), "1.9 GB");
    }

    #[test]
    fn test_list_files_empty_dir() {
        let cfg = Config::default_empty();
        // Point to a non-existent directory.
        let args = json!({
            "outputDirPath": "/tmp/nonexistent-mcp-test-dir"
        });
        let result = handle_list_files(&cfg, &args, None).unwrap();
        assert!(result.text.contains("does not exist"));
        assert_eq!(result.structured.unwrap()["total"], 0);
    }
}
