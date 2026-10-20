//! POST_EVENT support for tool calls.
//!
//! Tools that modify data (export, import, DDL, …) accept two optional
//! string parameters: `on_success_event` and `on_fail_event`. When the
//! handler completes and the corresponding parameter contains valid
//! non-empty text, two things happen:
//!
//! 1. A `POST_EVENT '<event_name>'` SQL statement is executed on the
//!    database, notifying the Firebird event manager. Clients registered
//!    for that event name (via `SET EVENT`) receive the notification on
//!    commit.
//! 2. A structured JSON line is appended to `{base}/events.jsonl` and
//!    logged via `mlog!` to the daily log file.
//!
//! Event emission must never fail the tool call: if the SQL statement or
//! the file write fails, the error is logged and the tool result is
//! returned unchanged.

use firebird_5_mcp_core::{FirebirdClient, mlog, paths, query};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;

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

/// Emits a POST_EVENT:
///
/// 1. Executes `POST_EVENT '<event_name>'` on the database (notifies the
///    Firebird event manager).
/// 2. Appends a JSON line to `{base}/events.jsonl`.
/// 3. Logs via `mlog!`.
///
/// Never fails the tool call — SQL and I/O errors are logged and dropped.
pub fn post_event(
    client: &mut FirebirdClient,
    event_name: &str,
    tool: &str,
    success: bool,
    database: Option<&str>,
    duration_ms: Option<u128>,
    detail: Option<&Value>,
    error: Option<&str>,
) {
    // 1. Firebird POST_EVENT (SQL)
    match query::post_event(client, event_name) {
        Ok(()) => mlog!("POST_EVENT '{event_name}' fired (tool={tool}, success={success})"),
        Err(e) => mlog!("POST_EVENT '{event_name}' FAILED (tool={tool}, success={success}): {e}"),
    }

    // 2. JSON line to events.jsonl
    let payload = build_event(
        event_name,
        tool,
        success,
        database,
        duration_ms,
        detail,
        error,
    );
    let line = serde_json::to_string(&payload).unwrap_or_default();
    append_event_line(&line);
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
}
