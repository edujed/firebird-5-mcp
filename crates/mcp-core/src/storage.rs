//! File storage for generated artifacts.
//!
//! Provides path validation, file writing, and preview generation for the
//! file-save feature. Generated files are written to a configured directory
//! with a subdirectory per database+format combination, enabling safe
//! overwriting (cache-like behavior) without timestamp suffixes.

use crate::codegen::targets::GeneratedFile;
use std::path::PathBuf;

/// Storage configuration from `config.json`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StorageConfig {
    /// Base directory for generated files.
    pub dir: String,
}

impl StorageConfig {
    /// Returns the effective storage directory.
    ///
    /// If `dir` is set, uses it. Otherwise, falls back to
    /// `{base}/output/` (see [`crate::paths::output_dir`]).
    pub fn effective_dir(&self) -> Option<PathBuf> {
        if !self.dir.is_empty() {
            return Some(PathBuf::from(&self.dir));
        }
        Some(crate::paths::output_dir())
    }
}

/// Returns the default storage directory: `{base}/output/`.
///
/// This is a persistent, predictable location that survives reboots and
/// OS temp cleanup. The directory is created on first write.
pub fn default_storage_dir() -> Option<PathBuf> {
    Some(crate::paths::output_dir())
}

/// Result of writing generated files to disk.
#[derive(Debug, Clone)]
pub struct StorageResult {
    /// Absolute path of the output directory.
    pub output_dir: String,
    /// Written files with their absolute paths.
    pub files: Vec<WrittenFile>,
}

/// A single written file with metadata.
#[derive(Debug, Clone)]
pub struct WrittenFile {
    /// File name (e.g. "User.java").
    pub name: String,
    /// Absolute path on disk.
    pub path: String,
    /// File size in bytes.
    pub size: usize,
    /// Number of lines.
    pub lines: usize,
    /// Output format (e.g. "java", "graphql-schema").
    pub format: String,
}

/// Validates and resolves the output directory for generated files.
///
/// The directory is `<base_dir>/<db_snake>_<format>/`. The base directory
/// must be absolute or relative to the current working directory. Path
/// traversal (`..`) is rejected.
pub fn resolve_output_dir(base_dir: &str, db_name: &str, format: &str) -> Result<PathBuf, String> {
    let db_snake = to_snake_case(db_name);
    let dir = PathBuf::from(base_dir).join(format!("{db_snake}_{format}"));

    // Reject path traversal
    if dir
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(format!("path traversal not allowed: {dir:?}"));
    }

    Ok(dir)
}

/// Writes generated files to disk and returns metadata for each.
///
/// Creates the output directory if it doesn't exist. Overwrites existing
/// files (cache-like behavior).
pub fn write_files(
    base_dir: &str,
    db_name: &str,
    format: &str,
    files: &[GeneratedFile],
) -> Result<StorageResult, String> {
    let dir = resolve_output_dir(base_dir, db_name, format)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create directory {dir:?}: {e}"))?;

    let mut written = Vec::new();
    for file in files {
        let path = dir.join(&file.name);
        std::fs::write(&path, &file.content)
            .map_err(|e| format!("failed to write {:?}: {e}", path))?;

        let size = file.content.len();
        let lines = file.content.lines().count();
        written.push(WrittenFile {
            name: file.name.clone(),
            path: path
                .to_str()
                .ok_or_else(|| format!("invalid path: {path:?}"))?
                .to_string(),
            size,
            lines,
            format: file.format.clone(),
        });
    }

    Ok(StorageResult {
        output_dir: dir
            .to_str()
            .ok_or_else(|| format!("invalid path: {dir:?}"))?
            .to_string(),
        files: written,
    })
}

/// Generates a preview of a file's content.
///
/// Returns the first `head` lines and last `tail` lines, separated by an
/// ellipsis. If the file has ≤ `head + tail` lines, returns the full content.
/// Lines longer than `max_line_len` are truncated with `…`.
pub fn preview(content: &str, head: usize, tail: usize, max_line_len: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if total <= head + tail {
        return truncate_lines(&lines, max_line_len).join("\n");
    }

    let mut out = Vec::with_capacity(head + tail + 1);
    for line in &lines[..head] {
        out.push(truncate_line(line, max_line_len));
    }
    out.push("...".to_string());
    for line in &lines[total - tail..] {
        out.push(truncate_line(line, max_line_len));
    }
    out.join("\n")
}

/// Generates a human-readable summary of written files.
///
/// Format:
/// ```text
/// 📄 3 files written to /path/to/output/
///    - User.java (2.3 KB, 58 lines)
///    - Order.java (1.8 KB, 45 lines)
///
/// Preview of User.java:
/// │ package com.app.entity;
/// │ import javax.persistence.*;
/// │ ...
/// ```
pub fn summary(
    result: &StorageResult,
    previews: &std::collections::HashMap<String, String>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "📄 {} file{} written to {}\n",
        result.files.len(),
        if result.files.len() == 1 { "" } else { "s" },
        result.output_dir
    ));

    for f in &result.files {
        out.push_str(&format!(
            "   - {} ({}, {} lines)\n",
            f.name,
            format_size(f.size),
            f.lines
        ));
    }

    // Include previews (limited to first 3 files to keep output manageable)
    for f in result.files.iter().take(3) {
        if let Some(pv) = previews.get(&f.name) {
            out.push('\n');
            out.push_str(&format!("Preview of {}:\n", f.name));
            for line in pv.lines() {
                out.push_str(&format!("│ {line}\n"));
            }
        }
    }

    out
}

/// Formats a byte size for display (e.g. "2.3 KB", "1.1 MB").
fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

fn truncate_lines(lines: &[&str], max_len: usize) -> Vec<String> {
    lines.iter().map(|l| truncate_line(l, max_len)).collect()
}

fn truncate_line(line: &str, max_len: usize) -> String {
    if line.len() <= max_len {
        line.to_string()
    } else {
        // Find the last char boundary at or before max_len
        let mut end = max_len;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &line[..end])
    }
}

/// Converts a string to snake_case (for directory names).
///
/// Handles: uppercase → lowercase, spaces/hyphens → underscores,
/// removes non-alphanumeric characters (except underscores).
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
        // Other characters are dropped
    }
    // Collapse multiple underscores and trim
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_underscore = false;
    for c in out.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push(c);
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }
    collapsed.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("prod"), "prod");
        assert_eq!(to_snake_case("ProdDB"), "prod_db");
        assert_eq!(to_snake_case("prod-db"), "prod_db");
        assert_eq!(to_snake_case("prod db"), "prod_db");
        assert_eq!(to_snake_case("MyProd.DB"), "my_prod_db");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("UPPER"), "upper");
    }

    #[test]
    fn test_preview_short_file() {
        let content = "line1\nline2\nline3";
        let pv = preview(content, 10, 5, 120);
        assert_eq!(pv, "line1\nline2\nline3");
    }

    #[test]
    fn test_preview_long_file() {
        let content: String = (1..=20)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let pv = preview(&content, 3, 2, 120);
        let lines: Vec<&str> = pv.lines().collect();
        assert_eq!(lines.len(), 6); // 3 + 1 (ellipsis) + 2
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");
        assert_eq!(lines[3], "...");
        assert_eq!(lines[4], "line19");
        assert_eq!(lines[5], "line20");
    }

    #[test]
    fn test_preview_truncates_long_lines() {
        let long_line = "a".repeat(200);
        let content = format!("{long_line}\nshort");
        let pv = preview(&content, 10, 5, 50);
        let first_line = pv.lines().next().unwrap();
        assert!(first_line.len() <= 53); // 50 + "…" (3 bytes in UTF-8)
        assert!(first_line.ends_with('…'));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn test_resolve_output_dir() {
        let dir = resolve_output_dir("/tmp/mcp-out", "ProdDB", "java").unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/mcp-out/prod_db_java"));
    }

    #[test]
    fn test_resolve_output_dir_rejects_traversal() {
        // Path traversal in the base directory is rejected
        let result = resolve_output_dir("/tmp/../etc", "prod", "java");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_storage_dir() {
        let dir = default_storage_dir();
        assert!(dir.is_some(), "home_dir should be available in test env");
        let dir = dir.unwrap();
        // Should end with .firebird-mcp/output
        let s = dir.to_str().unwrap();
        assert!(s.contains(".firebird-mcp"));
        assert!(s.ends_with("output") || s.ends_with("output\\"));
    }

    #[test]
    fn test_storage_config_effective_dir() {
        let cfg = StorageConfig {
            dir: "/custom/path".to_string(),
        };
        let dir = cfg.effective_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/custom/path"));

        let cfg_empty = StorageConfig { dir: String::new() };
        let dir = cfg_empty.effective_dir();
        assert!(dir.is_some(), "should fall back to default");
    }

    #[test]
    fn test_summary() {
        let result = StorageResult {
            output_dir: "/tmp/out".to_string(),
            files: vec![
                WrittenFile {
                    name: "User.java".to_string(),
                    path: "/tmp/out/User.java".to_string(),
                    size: 2350,
                    lines: 58,
                    format: "java".to_string(),
                },
                WrittenFile {
                    name: "Order.java".to_string(),
                    path: "/tmp/out/Order.java".to_string(),
                    size: 1820,
                    lines: 45,
                    format: "java".to_string(),
                },
            ],
        };
        let mut previews = std::collections::HashMap::new();
        previews.insert(
            "User.java".to_string(),
            "package com.app;\n// ...".to_string(),
        );
        let s = summary(&result, &previews);
        assert!(s.contains("2 files written to /tmp/out"));
        assert!(s.contains("User.java (2.3 KB, 58 lines)"));
        assert!(s.contains("Order.java (1.8 KB, 45 lines)"));
        assert!(s.contains("Preview of User.java:"));
    }
}
