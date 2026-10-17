//! MCP stdio server for Firebird.
//!
//! Implements the MCP protocol (JSON-RPC 2.0, line-delimited) and exposes the
//! same tool set as the Go implementation, plus the new Rust-only tools
//! (`firebird_explain_plan`, `firebird_create_procedure`,
//! `firebird_create_function`, `firebird_create_sequence`).

use firebird_5_mcp_core::{Config, mlog};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::sync::RwLock;

mod tools;

use tools::{ToolResult, dispatch, tool_definitions};

/// Shared, reloadable configuration.
///
/// The config is loaded once at startup and can be refreshed by
/// `firebird_get_databases` (which re-reads the file from disk) or updated
/// in-memory by `firebird_create_database` (after registering a new db).
static CONFIG: RwLock<Option<Config>> = RwLock::new(None);

const SERVER_NAME: &str = "jed-personal-mcp";
const SERVER_VERSION: &str = "1.0.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Build timestamp, generated at compile time using `chrono`.
/// This is evaluated once when the binary is compiled.
pub fn build_time() -> &'static str {
    use std::sync::OnceLock;
    static BUILD_TIME: OnceLock<String> = OnceLock::new();
    BUILD_TIME.get_or_init(|| {
        // Use a fixed timestamp for reproducibility in CI/CD
        // In local builds, this will be the actual compile time
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    })
}

fn main() {
    let cfg = match Config::load_or_create() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    // Initialize file logging to {base}/logs/ (see paths module)
    let log_dir = firebird_5_mcp_core::paths::logs_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    firebird_5_mcp_core::log::init(log_dir.to_str().unwrap());
    mlog!("MCP server started (build {})", build_time());
    mlog!(
        "config: host={} port={} user={} dbs=[{}] allowed_paths=[{}]",
        cfg.server.host,
        cfg.server.port,
        cfg.server.user,
        cfg.server.database_names(),
        cfg.server.allowed_paths.join(", ")
    );

    // Store in the shared RwLock for tool handlers
    *CONFIG.write().unwrap() = Some(cfg);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                mlog!("ignoring malformed JSON line: {e}");
                continue;
            }
        };

        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Notifications (no `id`) must not produce a response. The MCP client
        // sends `notifications/initialized` right after `initialize`; per the
        // spec we acknowledge it silently. Other notifications are ignored.
        if request.get("id").is_none() {
            if method == "notifications/initialized" {
                mlog!("initialized");
            }
            continue;
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
                let cfg_guard = CONFIG.read().unwrap();
                let cfg = cfg_guard.as_ref().unwrap();
                match dispatch(cfg, name, &args) {
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

        if let Ok(response) = response {
            let mut out = response.to_string();
            out.push('\n');
            if writer.write_all(out.as_bytes()).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    }
}
