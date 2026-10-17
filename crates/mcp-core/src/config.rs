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

use crate::storage::StorageConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single configured database.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub path: String,
}

/// Firebird server connection details and the list of databases.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    pub isql_path: Option<String>,
}

/// Root structure of `config.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    /// Optional storage configuration for file-save feature.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,
}

impl Config {
    /// Reads and validates the configuration file.
    /// Lookup order: `FIREBIRD_CONFIG` env var, then `./config.json`,
    /// then `~/.firebird-mcp/config.json`.
    pub fn load() -> Result<Self, String> {
        let path = config_path();
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read config file ({}): {e}", path.display()))?;

        let cfg: Config =
            serde_json::from_str(&data).map_err(|e| format!("failed to parse config file: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Loads the config, or creates a default one at the fallback path
    /// (`~/.firebird-mcp/config.json`) if no config file **exists**.
    ///
    /// Important: this only creates a new file when the config file is
    /// missing. It will NEVER overwrite an existing file, even if the
    /// file contains an empty database list or is being edited.
    pub fn load_or_create() -> Result<Self, String> {
        // Try existing config first
        if let Ok(cfg) = Self::load() {
            return Ok(cfg);
        }

        // Load failed — check if the file exists (but is invalid/empty)
        let path = config_path();
        if path.exists() {
            // File exists but failed to load (e.g. empty databases list).
            // Do NOT overwrite — return the parsed config without strict validation.
            let data = std::fs::read_to_string(&path)
                .map_err(|e| format!("failed to read config file ({}): {e}", path.display()))?;
            let cfg: Config = serde_json::from_str(&data)
                .map_err(|e| format!("failed to parse config file: {e}"))?;
            return Ok(cfg);
        }

        // File truly missing — create a default at the fallback path
        let path = default_config_path()
            .ok_or_else(|| "cannot determine home directory for default config".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create config directory {:?}: {e}", parent))?;
        }

        let cfg = Config::default_empty();
        cfg.save_to(&path)
            .map_err(|e| format!("failed to write default config to {:?}: {e}", path))?;
        Ok(cfg)
    }

    /// Creates a minimal default config (no databases).
    pub fn default_empty() -> Self {
        Config {
            server: ServerConfig {
                host: "localhost".to_string(),
                port: 3050,
                user: "SYSDBA".to_string(),
                password: "masterkey".to_string(),
                allowed_paths: Vec::new(),
                databases: Vec::new(),
                default_database: None,
                isql_path: None,
            },
            storage: None,
        }
    }

    /// Saves the config to the path it was loaded from (or the default path).
    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        self.save_to(&path)
    }

    /// Saves the config to a specific path.
    pub fn save_to(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create config directory {:?}: {e}", parent))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize config: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("failed to write config file {:?}: {e}", path))
    }

    /// Registers a database in the config and saves it to disk.
    ///
    /// If a database with the same name already exists (case-insensitive),
    /// its path and description are updated. Otherwise, a new entry is
    /// appended. If this is the first database, it becomes the default.
    pub fn register_database(
        &mut self,
        name: &str,
        path: &str,
        description: Option<&str>,
    ) -> Result<(), String> {
        let name = name.to_string();
        let path = path.to_string();
        let description = description.map(|s| s.to_string());

        // Check if already registered (case-insensitive)
        if let Some(existing) = self
            .server
            .databases
            .iter_mut()
            .find(|db| db.name.eq_ignore_ascii_case(&name))
        {
            existing.path = path;
            existing.description = description;
        } else {
            self.server.databases.push(DatabaseConfig {
                name: name.clone(),
                description,
                path,
            });
            // First database becomes the default
            if self.server.default_database.is_none() {
                self.server.default_database = Some(name);
            }
        }

        self.save()
    }

    fn validate(&self) -> Result<(), String> {
        // Only validate default_database if there are databases configured.
        // An empty database list is valid (the MCP can start without databases
        // and have them registered later via firebird_create_database).
        if self.server.databases.is_empty() {
            return Ok(());
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
    /// Returns an empty string if no databases are configured.
    pub fn default_database(&self) -> &str {
        self.server
            .default_database
            .as_deref()
            .or_else(|| self.server.databases.first().map(|d| d.name.as_str()))
            .unwrap_or("")
    }

    /// Re-reads the configuration file from disk.
    ///
    /// This is used by `firebird_get_databases` to pick up changes made
    /// externally (e.g. by `firebird_create_database` in a previous session,
    /// or by the user editing the file directly).
    pub fn reload() -> Result<Self, String> {
        let path = config_path();
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read config file ({}): {e}", path.display()))?;
        let cfg: Config =
            serde_json::from_str(&data).map_err(|e| format!("failed to parse config file: {e}"))?;
        Ok(cfg)
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

/// Resolves the config file path.
///
/// Lookup order:
/// 1. `FIREBIRD_CONFIG` environment variable
/// 2. `./config.json` (current working directory)
/// 3. `{base}/config.json` (see [`crate::paths::config_file`])
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("FIREBIRD_CONFIG") {
        return PathBuf::from(p);
    }
    let local = PathBuf::from("config.json");
    if local.exists() {
        return local;
    }
    crate::paths::config_file()
}

/// Returns the default config path: `{base}/config.json`.
pub fn default_config_path() -> Option<PathBuf> {
    Some(crate::paths::config_file())
}
