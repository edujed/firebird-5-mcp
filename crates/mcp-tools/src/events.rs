//! POST_EVENT support for tool calls.
//!
//! Every tool call accepts two optional string parameters:
//! `on_success_event` and `on_fail_event`. After the handler completes,
//! [`emit_tool_event`] (called from `dispatch()`) checks the relevant
//! parameter and, if it contains valid non-empty text:
//!
//! 1. Appends a structured JSON line to `{base}/events.jsonl` (always).
//! 2. If the call targeted a valid database, opens a new connection and
//!    executes `POST_EVENT '<event_name>'` in its own transaction. When the
//!    transaction commits, the Firebird event manager notifies all clients
//!    registered for that event name (via `SET EVENT`).
//!
//! Event emission must never fail the tool call: if the SQL statement or
//! the file write fails, the error is logged and the tool result is
//! returned unchanged.

use firebird_5_mcp_core::{Config, FirebirdClient, mlog, paths, query};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;

use crate::common::arg_str;

/// Path to the events file: `{base}/events.jsonl`.
pub fn events_file() -> PathBuf {
    paths::base_dir().join("events.jsonl")
}

/// Returns `Some(trimmed)` if the event name is valid (present, non-empty
/// after trim), `None` otherwise.
pub fn valid_event_name(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Builds the structured event payload.
///
/// Fields: `event`, `tool`, `success`, `database`, `timestamp`,
/// `duration_ms` (optional), `detail` (optional, the handler's structured
/// result), `error` (optional, the error message).
pub fn build_event(
    event_name: &str,
    tool: &str,
    success: bool,
    database: Option<&str>,
    duration_ms: Option<u128>,
    detail: Option<&Value>,
    error: Option<&str>,
) -> Value {
    let mut payload = json!({
        "event": event_name,
        "tool": tool,
        "success": success,
        "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string(),
    });
    if let Some(db) = database {
        payload["database"] = json!(db);
    }
    if let Some(ms) = duration_ms {
        payload["duration_ms"] = json!(ms);
    }
    if let Some(d) = detail {
        payload["detail"] = d.clone();
    }
    if let Some(e) = error {
        payload["error"] = json!(e);
    }
    payload
}

/// Resolves the database name for a tool call: the `database` argument if
/// present, otherwise the configured default. Returns `None` when no
/// database can be resolved (e.g. `firebird_get_databases` has no
/// `database` argument and no default is configured).
pub fn resolve_database(cfg: &Config, args: &Value) -> Option<String> {
    let name = arg_str(args, "database").or_else(|| {
        let d = cfg.default_database();
        (!d.is_empty()).then(|| d.to_string())
    })?;
    // Validate that the database is actually configured.
    cfg.server
        .find_database(&name)
        .ok()
        .map(|db| db.name.clone())
}

/// Emits a tool event after a handler completes.
///
/// This is the universal entry point called from `dispatch()`. It:
/// 1. Appends a JSON line to `{base}/events.jsonl` (always).
/// 2. If the call targeted a valid database, opens a **new** connection and
///    executes `POST_EVENT '<event_name>'` in its own transaction (the
///    handler's connection may be in an unknown state after a failure).
///
/// Never fails — all errors are logged via `mlog!` and dropped.
pub fn emit_tool_event(
    cfg: &Config,
    tool: &str,
    args: &Value,
    success: bool,
    duration_ms: u128,
    detail: Option<&Value>,
    error: Option<&str>,
) {
    let event_name = if success {
        valid_event_name(arg_str(args, "on_success_event").as_deref())
    } else {
        valid_event_name(arg_str(args, "on_fail_event").as_deref())
    };
    let Some(event_name) = event_name else {
        return;
    };

    let db_name = resolve_database(cfg, args);

    // 1. JSON line to events.jsonl (always)
    let payload = build_event(
        &event_name,
        tool,
        success,
        db_name.as_deref(),
        Some(duration_ms),
        detail,
        error,
    );
    let line = serde_json::to_string(&payload).unwrap_or_default();
    append_event_line(&line);
    mlog!(
        "POST_EVENT '{}' logged (tool={}, success={}, db={})",
        event_name,
        tool,
        success,
        db_name.as_deref().unwrap_or("-")
    );

    // 2. Firebird POST_EVENT (only when a valid database is resolved)
    if let Some(ref db) = db_name {
        match open_client_for_event(cfg, db) {
            Ok(mut client) => match query::post_event(&mut client, &event_name) {
                Ok(()) => mlog!(
                    "POST_EVENT '{}' fired (tool={}, success={})",
                    event_name,
                    tool,
                    success
                ),
                Err(e) => mlog!(
                    "POST_EVENT '{}' FAILED (tool={}, success={}): {e}",
                    event_name,
                    tool,
                    success
                ),
            },
            Err(e) => mlog!(
                "POST_EVENT '{}' could not open connection to '{}': {e}",
                event_name,
                db
            ),
        }
    }
}

/// Opens a fresh client for the given database (used for POST_EVENT).
fn open_client_for_event(cfg: &Config, db_name: &str) -> Result<FirebirdClient, String> {
    let db = cfg
        .server
        .find_database(db_name)
        .map_err(|e| e.to_string())?;
    FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())
}

/// Appends a single JSON line to the events file, creating the base
/// directory if needed. Silently drops on any I/O error.
fn append_event_line(line: &str) {
    let path = events_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_event_name() {
        assert_eq!(valid_event_name(None), None);
        assert_eq!(valid_event_name(Some("")), None);
        assert_eq!(valid_event_name(Some("   ")), None);
        assert_eq!(
            valid_event_name(Some("export_done")),
            Some("export_done".to_string())
        );
        assert_eq!(valid_event_name(Some("  done  ")), Some("done".to_string()));
    }

    #[test]
    fn test_build_event_success() {
        let detail = json!({"rows": 42, "path": "/tmp/out.parquet"});
        let ev = build_event(
            "export_done",
            "firebird_export",
            true,
            Some("prod"),
            Some(1500),
            Some(&detail),
            None,
        );
        assert_eq!(ev["event"], "export_done");
        assert_eq!(ev["tool"], "firebird_export");
        assert_eq!(ev["success"], true);
        assert_eq!(ev["database"], "prod");
        assert_eq!(ev["duration_ms"], 1500);
        assert_eq!(ev["detail"]["rows"], 42);
        assert!(ev.get("error").is_none());
        assert!(ev["timestamp"].as_str().is_some());
    }

    #[test]
    fn test_build_event_failure() {
        let ev = build_event(
            "import_failed",
            "firebird_import",
            false,
            None,
            None,
            None,
            Some("table already exists"),
        );
        assert_eq!(ev["event"], "import_failed");
        assert_eq!(ev["success"], false);
        assert!(ev.get("database").is_none());
        assert!(ev.get("duration_ms").is_none());
        assert!(ev.get("detail").is_none());
        assert_eq!(ev["error"], "table already exists");
    }

    #[test]
    fn test_append_event_line() {
        let dir = std::env::temp_dir().join(format!("mcp-events-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var(paths::ENV_HOME, &dir);
        }

        append_event_line(r#"{"event":"test","success":true}"#);
        append_event_line(r#"{"event":"test2","success":false}"#);

        let content = std::fs::read_to_string(events_file()).expect("events.jsonl should exist");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["event"], "test");
        assert_eq!(first["success"], true);

        let second: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["event"], "test2");
        assert_eq!(second["success"], false);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_emit_tool_event_no_event_param() {
        // No on_success_event / on_fail_event → nothing happens, no panic.
        let cfg = Config::default_empty();
        let args = json!({"source": "T", "output_path": "/tmp/x"});
        emit_tool_event(&cfg, "firebird_export", &args, true, 100, None, None);
        emit_tool_event(
            &cfg,
            "firebird_import",
            &args,
            false,
            100,
            None,
            Some("err"),
        );
        // No events.jsonl should have been created (or it's empty).
    }

    #[test]
    fn test_emit_tool_event_success_writes_jsonl() {
        let dir = std::env::temp_dir().join(format!("mcp-events-test2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var(paths::ENV_HOME, &dir);
        }

        let cfg = Config::default_empty();
        let args = json!({"on_success_event": "test_ok"});
        emit_tool_event(
            &cfg,
            "firebird_export",
            &args,
            true,
            42,
            Some(&json!({"rows": 1})),
            None,
        );

        let content = std::fs::read_to_string(events_file()).expect("events.jsonl should exist");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        let ev: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(ev["event"], "test_ok");
        assert_eq!(ev["success"], true);
        assert_eq!(ev["duration_ms"], 42);
        assert_eq!(ev["detail"]["rows"], 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_emit_tool_event_fail_writes_jsonl() {
        let dir = std::env::temp_dir().join(format!("mcp-events-test3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var(paths::ENV_HOME, &dir);
        }

        let cfg = Config::default_empty();
        let args = json!({"on_fail_event": "test_fail"});
        emit_tool_event(
            &cfg,
            "firebird_import",
            &args,
            false,
            17,
            None,
            Some("boom"),
        );

        let content = std::fs::read_to_string(events_file()).expect("events.jsonl should exist");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        let ev: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(ev["event"], "test_fail");
        assert_eq!(ev["success"], false);
        assert_eq!(ev["error"], "boom");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
