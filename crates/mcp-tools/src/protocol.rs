//! Transport-agnostic MCP protocol core.
//!
//! This module encapsulates the JSON-RPC 2.0 request handling that is
//! independent of the underlying transport (stdio, HTTP/SSE, etc.). It
//! holds the shared, reloadable configuration and routes incoming
//! requests to the appropriate handler.
//!
//! The consuming crates (`mcp-stdio-bin` for stdio, and the future
//! `mcp-http-bin` for HTTP/SSE) are responsible for reading requests from
//! their respective transports and writing responses back. This module
//! only transforms a request into a response.

use crate::{ToolResult, build_time, dispatch, tool_definitions};
use firebird_5_mcp_core::Config;
use serde_json::{Value, json};
use std::sync::RwLock;

/// MCP protocol version supported by this server.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server name reported in the `initialize` response.
pub const SERVER_NAME: &str = "jed-personal-mcp";

/// Server version reported in the `initialize` response.
pub const SERVER_VERSION: &str = "1.0.0";

/// Shared, reloadable configuration plus the protocol routing logic.
///
/// The config is loaded once at startup and can be refreshed by
/// `firebird_get_databases` (which re-reads the file from disk) or updated
/// in-memory by `firebird_create_database` (after registering a new db).
///
/// `ProtocolCore` is designed to be shared across threads (e.g. one
/// instance per SSE client, or a single instance behind an async runtime),
/// so all mutable state is behind an `RwLock`.
pub struct ProtocolCore {
    config: RwLock<Option<Config>>,
    /// Base URL of the `/files` HTTP endpoint (e.g. `http://192.168.1.50:8080`),
    /// set only in SSE mode. When present, file-generating tools include a
    /// `download_url` in their structured response. `None` in stdio mode.
    files_base_url: Option<String>,
}

impl ProtocolCore {
    /// Creates a new protocol core with the given initial configuration.
    ///
    /// `files_base_url` is `None` for stdio mode (no HTTP endpoint) and
    /// `Some("http://host:port")` for SSE mode, so that file-generating tools
    /// can advertise a `download_url` in their responses.
    pub fn new(cfg: Config, files_base_url: Option<String>) -> Self {
        Self {
            config: RwLock::new(Some(cfg)),
            files_base_url,
        }
    }

    /// Returns the base URL of the `/files` endpoint, if any (SSE mode).
    pub fn files_base_url(&self) -> Option<&str> {
        self.files_base_url.as_deref()
    }

    /// Returns a read guard on the current configuration.
    ///
    /// Panics if the configuration has not been set (should not happen in
    /// normal operation, since `new` always provides one).
    pub fn config(&self) -> std::sync::RwLockReadGuard<'_, Option<Config>> {
        self.config.read().unwrap()
    }

    /// Replaces the current configuration.
    ///
    /// Used by `firebird_create_database` to register a new database
    /// in-memory after writing it to disk.
    pub fn set_config(&self, cfg: Config) {
        *self.config.write().unwrap() = Some(cfg);
    }

    /// Processes a single JSON-RPC 2.0 request and returns the response.
    ///
    /// Returns `None` for notifications (requests without an `id`), which
    /// per the MCP spec must not produce a response. The caller is
    /// responsible for silently acknowledging them.
    ///
    /// The request is expected to be a valid JSON-RPC 2.0 object. Malformed
    /// requests (missing `method`, unknown methods, etc.) are handled
    /// internally and produce an appropriate JSON-RPC error response.
    pub fn handle_request(&self, request: &Value) -> Option<Value> {
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Notifications (no `id`) must not produce a response. The MCP client
        // sends `notifications/initialized` right after `initialize`; per the
        // spec we acknowledge it silently. Other notifications are ignored.
        if request.get("id").is_none() {
            if method == "notifications/initialized" {
                firebird_5_mcp_core::mlog!("initialized");
            }
            return None;
        }

        let id = request.get("id").cloned();
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let response: Result<Value, ()> = match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION,
                        "buildTime": build_time()
                    }
                });
                Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            }
            "ping" => Ok(json!({ "jsonrpc": "2.0", "id": id, "result": json!({}) })),
            "tools/list" => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tool_definitions() }
            })),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                // Get a read guard on the config for this tool call
                let cfg_guard = self.config.read().unwrap();
                let cfg = cfg_guard.as_ref().unwrap();
                match dispatch(cfg, name, &args, self.files_base_url.as_deref()) {
                    Ok(ToolResult { text, structured }) => {
                        let mut result = json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": false
                        });
                        if let Some(s) = structured {
                            result["structuredContent"] = s;
                        }
                        Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                    }
                    Err(e) => {
                        let result = json!({
                            "content": [{ "type": "text", "text": e }],
                            "isError": true
                        });
                        Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                    }
                }
            }
            _ => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })),
        };

        response.ok()
    }
}
