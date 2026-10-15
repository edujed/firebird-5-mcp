//! MCP stdio server for Firebird.
//!
//! Implements the MCP protocol (JSON-RPC 2.0, line-delimited) and exposes the
//! same tool set as the Go implementation, plus the new Rust-only tools
//! (`firebird_explain_plan`, `firebird_create_procedure`,
//! `firebird_create_function`, `firebird_create_sequence`).

use firebird_5_mcp_core::Config;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

mod tools;

use tools::{dispatch, tool_definitions};

const SERVER_NAME: &str = "jed-personal-mcp";
const SERVER_VERSION: &str = "1.0.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let cfg = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

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
            Err(_) => continue,
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let response: Result<Value, ()> = match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
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
                match dispatch(&cfg, name, &args) {
                    Ok(text) => {
                        let result = json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": false
                        });
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
