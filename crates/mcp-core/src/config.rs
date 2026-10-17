//! Configuration loading for the Firebird MCP server.
//!
//! The on-disk format is identical to the Go implementation (`config.json`):
//!
//! ```json
//! {
//!   "server": {
//!     "host": "localhost",
//!     "port": 3050,
//!     "user": "SYSDBA",
//!     "password": "masterkey",
//!     "allowed_paths": ["/database"],
//!     "databases": [
//!       { "name": "teste", "description": "...", "path": "/database/teste.fdb" }
//!     ],
//!     "default_database": "teste"
//!   }
//! }
//! ```
//!
//! Lookup order: `FIREBIRD_CONFIG` env var, then `./config.json`.
//! The legacy `isql_path` key is accepted (for compatibility) but ignored —
//! the Rust implementation no longer shells out to isql.

use serde::Deserialize;
use std::path::PathBuf;

/// A single configured database.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub path: String,
}

/// Firebird server connection details and the list of databases.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    pub databases: Vec<DatabaseConfig>,
    #[serde(default)]
    pub default_database: Option<String>,
    /// Legacy key from the Go config. Accepted for compatibility, ignored.
    #[serde(default)]
    #[allow(dead_code)]
    pub isql_path: Option<String>,
}

/// Root structure of `config.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
}

impl Config {
    /// Reads and validates the configuration file.
    /// Lookup order: `FIREBIRD_CONFIG` env var, then `./config.json`.
    pub fn load() -> Result<Self, String> {
        let path = std::env::var("FIREBIRD_CONFIG").unwrap_or_else(|_| "config.json".to_string());
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read config file ({path}): {e}"))?;

        let cfg: Config =
            serde_json::from_str(&data).map_err(|e| format!("failed to parse config file: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), String> {
        if self.server.databases.is_empty() {
            return Err("no databases configured".into());
        }
        let default = self
            .server
            .default_database
            .as_deref()
            .unwrap_or(self.server.databases[0].name.as_str());
        if !self
            .server
            .databases
            .iter()
            .any(|db| db.name.eq_ignore_ascii_case(default))
        {
            return Err(format!(
                "default_database '{default}' not found in database list"
            ));
        }
        Ok(())
    }

    /// The effective default database name (first entry if not set).
    pub fn default_database(&self) -> &str {
        self.server
            .default_database
            .as_deref()
            .unwrap_or(self.server.databases[0].name.as_str())
    }
}

impl ServerConfig {
    /// Looks up a database by name (case-insensitive).
    pub fn find_database(&self, name: &str) -> Result<&DatabaseConfig, String> {
        self.databases
            .iter()
            .find(|db| db.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                format!(
                    "database '{name}' not found. Available: {}",
                    self.database_names()
                )
            })
    }

    /// Comma-separated list of configured database names.
    pub fn database_names(&self) -> String {
        self.databases
            .iter()
            .map(|db| db.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Checks if the given path is inside one of the allowed directories.
    pub fn is_path_allowed(&self, path: &str) -> bool {
        self.allowed_paths.iter().any(|dir| {
            let prefix = if dir.ends_with('/') {
                dir.clone()
            } else {
                format!("{dir}/")
            };
            path.starts_with(&prefix)
        })
    }
}

/// Resolves the config file path the same way the Go version does.
pub fn config_path() -> PathBuf {
    std::env::var("FIREBIRD_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config.json"))
}
