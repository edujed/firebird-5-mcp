//! Centralized path resolution for the Firebird MCP server.
//!
//! All file paths used by the MCP server are derived from a single base
//! directory, ensuring a consistent layout:
//!
//! ```text
//! ~/.firebird-mcp/          (or $FIREBIRD_MCP_HOME)
//! ├── config.json           — server configuration
//! ├── output/               — generated code files
//! └── logs/                 — daily log files (yyyy-mm-dd.log)
//! ```
//!
//! Override the base directory with the `FIREBIRD_MCP_HOME` environment
//! variable (useful for testing or non-standard installs).

use std::path::PathBuf;

/// Environment variable that overrides the MCP base directory.
pub const ENV_HOME: &str = "FIREBIRD_MCP_HOME";

/// The default base directory name (under the user's home).
const BASE_DIR_NAME: &str = ".firebird-mcp";

/// Returns the MCP base directory.
///
/// Resolution order:
/// 1. `FIREBIRD_MCP_HOME` environment variable
/// 2. `~/.firebird-mcp`
/// 3. `/tmp/firebird-mcp` (last resort if home is unavailable)
pub fn base_dir() -> PathBuf {
    if let Ok(custom) = std::env::var(ENV_HOME) {
        return PathBuf::from(custom);
    }
    dirs::home_dir()
        .map(|home| home.join(BASE_DIR_NAME))
        .unwrap_or_else(|| PathBuf::from("/tmp/firebird-mcp"))
}

/// Path to the configuration file: `{base}/config.json`.
pub fn config_file() -> PathBuf {
    base_dir().join("config.json")
}

/// Path to the output directory for generated files: `{base}/output/`.
pub fn output_dir() -> PathBuf {
    base_dir().join("output")
}

/// Path to the logs directory: `{base}/logs/`.
pub fn logs_dir() -> PathBuf {
    base_dir().join("logs")
}

/// Ensures the base directory and all standard subdirectories exist.
///
/// Creates:
/// - `{base}/`
/// - `{base}/output/`
/// - `{base}/logs/`
///
/// Returns the base directory path.
pub fn ensure_dirs() -> PathBuf {
    let base = base_dir();
    let _ = std::fs::create_dir_all(&base);
    let _ = std::fs::create_dir_all(output_dir());
    let _ = std::fs::create_dir_all(logs_dir());
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir_fallback() {
        // Without env var set, should resolve to ~/.firebird-mcp or /tmp/firebird-mcp
        let dir = base_dir();
        assert!(
            dir.to_str().unwrap().contains(".firebird-mcp")
                || dir.to_str().unwrap().contains("firebird-mcp"),
            "unexpected base dir: {}",
            dir.display()
        );
    }

    #[test]
    fn test_config_file_ends_with_json() {
        assert!(config_file().to_str().unwrap().ends_with("config.json"));
    }

    #[test]
    fn test_output_dir_subpath() {
        let out = output_dir();
        assert!(out.starts_with(base_dir()));
        assert!(out.to_str().unwrap().ends_with("output"));
    }

    #[test]
    fn test_logs_dir_subpath() {
        let logs = logs_dir();
        assert!(logs.starts_with(base_dir()));
        assert!(logs.to_str().unwrap().ends_with("logs"));
    }
}
