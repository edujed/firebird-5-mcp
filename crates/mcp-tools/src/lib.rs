//! # firebird-5-mcp-tools
//!
//! Shared MCP tool handlers (transport-agnostic).
//!
//! This crate contains the **business logic** of the Firebird MCP tools:
//! the handlers that parse JSON arguments, call into `firebird-5-mcp-core`,
//! and return a Markdown-formatted [`ToolResult`]. It has **no dependency
//! on any transport protocol** (stdio, HTTP, SSE) — that is the job of the
//! consuming crates (`firebird-5-mcp` for stdio, and the future
//! `firebird-5-mcp-http` for HTTP/SSE).
//!
//! ## What lives here
//!
//! - [`ToolResult`] — the envelope returned by every tool call.
//! - [`dispatch`] — routes a tool name + JSON args to the right handler.
//! - [`tool_definitions`] — the JSON schemas for `tools/list`.
//! - One module per tool group: [`query_tools`], [`schema_tools`],
//!   [`stats_tools`], [`introspection_tools`], [`generator_tools`],
//!   [`view_dto_tools`], [`ddl_tools`].
//! - [`common`] — shared helpers (arg parsing, client opening, Markdown
//!   rendering, file saving).
//!
//! ## What does not live here
//!
//! - The MCP protocol loop (JSON-RPC over stdio) — lives in `firebird-5-mcp`.
//! - The HTTP/SSE server — will live in `firebird-5-mcp-http`.
//! - Database driver, query execution, schema introspection, DDL, codegen —
//!   live in `firebird-5-mcp-core`.
//! - Value types (`Value`, `QueryResult`, `format_column_type`) — live in
//!   `firebird-5-mcp-types`.

use firebird_5_mcp_core::{Config, mlog};
use serde_json::Value;

/// Build timestamp, generated at compile time.
/// This is evaluated once when the crate is compiled.
pub fn build_time() -> &'static str {
    use std::sync::OnceLock;
    static BUILD_TIME: OnceLock<String> = OnceLock::new();
    BUILD_TIME.get_or_init(|| chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
}

pub mod common;
pub mod ddl_tools;
pub mod definitions;
pub mod events;
pub mod file_tools;
pub mod generator_tools;
pub mod introspection_tools;
pub mod migration_tools;
pub mod protocol;
pub mod quality_tools;
pub mod query_tools;
pub mod schema_tools;
pub mod stats_tools;
pub mod view_dto_tools;

pub use ddl_tools::*;
pub use definitions::*;
pub use events::*;
pub use file_tools::*;
pub use generator_tools::*;
pub use introspection_tools::*;
pub use migration_tools::*;
pub use protocol::{PROTOCOL_VERSION, ProtocolCore, SERVER_NAME, SERVER_VERSION};
pub use quality_tools::*;
pub use query_tools::*;
pub use schema_tools::*;
pub use stats_tools::*;
pub use view_dto_tools::*;

/// Result of a tool call.
///
/// `text` is the human-readable response (always present).
/// `structured` is optional structured data (e.g. file paths, sizes) that
/// clients can parse without regex.
pub struct ToolResult {
    pub text: String,
    pub structured: Option<Value>,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            structured: None,
        }
    }

    pub fn with_structured(text: impl Into<String>, structured: Value) -> Self {
        Self {
            text: text.into(),
            structured: Some(structured),
        }
    }
}

/// Wraps a `Result<String, String>` into a `Result<ToolResult, String>`.
/// Used for handlers that don't support file-save yet.
fn wrap_text(result: Result<String, String>) -> Result<ToolResult, String> {
    result.map(ToolResult::text)
}

/// Dispatches a tool call to the appropriate handler.
///
/// After the handler completes, if the call included a valid
/// `on_success_event` (on success) or `on_fail_event` (on failure), a
/// POST_EVENT is emitted: a JSON line is appended to `events.jsonl` and,
/// when a valid database is resolved, `POST_EVENT '<name>'` is executed on
/// the database in a fresh transaction.
pub fn dispatch(
    cfg: &Config,
    name: &str,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    mlog!("tool call: {name}");
    let started = std::time::Instant::now();
    let result = dispatch_inner(cfg, name, args, files_base_url);
    let duration_ms = started.elapsed().as_millis();

    if let Err(ref e) = result {
        mlog!("tool {name} ERROR: {e}");
    }

    // Universal POST_EVENT: fires for any tool that has a valid event param
    // and a resolvable database. See `events::emit_tool_event`.
    let success = result.is_ok();
    let detail = result.as_ref().ok().and_then(|r| r.structured.as_ref());
    let error = result.as_ref().err().map(|s| s.as_str());
    crate::events::emit_tool_event(cfg, name, args, success, duration_ms, detail, error);

    result
}

fn dispatch_inner(
    cfg: &Config,
    name: &str,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    match name {
        // Query tools
        "firebird_query" => wrap_text(handle_query(cfg, args)),
        "firebird_count" => wrap_text(handle_count(cfg, args)),
        "firebird_sample" => wrap_text(handle_sample(cfg, args)),
        "firebird_insert_batch" => wrap_text(handle_insert_batch(cfg, args)),
        "firebird_run_script" => wrap_text(handle_run_script(cfg, args)),
        "firebird_execute_immediate" => wrap_text(handle_execute_immediate(cfg, args)),
        "firebird_explain_plan" => wrap_text(handle_explain_plan(cfg, args)),

        // Schema tools
        "firebird_describe_table" => wrap_text(handle_describe_table(cfg, args)),
        "firebird_describe_database" => wrap_text(handle_describe_database(cfg, args)),
        "firebird_describe" => wrap_text(handle_describe(cfg, args)),
        "firebird_list" => wrap_text(handle_list(cfg, args)),
        "firebird_find_column" => wrap_text(handle_find_column(cfg, args)),
        "firebird_check_duplicates" => wrap_text(handle_check_duplicates(cfg, args)),
        "firebird_domains" => wrap_text(handle_domains(cfg, args)),
        "firebird_get_databases" => wrap_text(handle_get_databases(cfg)),
        "firebird_metadata_extract" => handle_metadata_extract(cfg, args, files_base_url),

        // Stats tools
        "firebird_crypt_state" => wrap_text(handle_crypt_state(cfg, args)),
        "firebird_database_stats" => wrap_text(handle_database_stats(cfg, args)),
        "firebird_query_history" => wrap_text(handle_query_history(cfg, args)),

        // Introspection tools
        "firebird_table_stats" => wrap_text(handle_table_stats(cfg, args)),
        "firebird_users" => wrap_text(handle_users(cfg, args)),
        "firebird_grants" => wrap_text(handle_grants(cfg, args)),
        "firebird_blob_info" => wrap_text(handle_blob_info(cfg, args)),
        "firebird_index_usage" => wrap_text(handle_index_usage(cfg, args)),

        // Generator tools (support file-save)
        "firebird_generate_dev" => handle_generate_dev(cfg, args, files_base_url),
        "firebird_generate_view_dto" => handle_generate_view_dto(cfg, args, files_base_url),

        // DDL tools
        "firebird_create_database" => handle_create_database(cfg, args),
        "firebird_create_trigger" => wrap_text(handle_create_trigger(cfg, args)),
        "firebird_create_procedure" => wrap_text(handle_create_procedure(cfg, args)),
        "firebird_create_function" => wrap_text(handle_create_function(cfg, args)),
        "firebird_create_sequence" => wrap_text(handle_create_sequence(cfg, args)),
        "firebird_alter_column_type" => wrap_text(handle_alter_column_type(cfg, args)),
        "firebird_drop" => wrap_text(handle_drop(cfg, args)),

        // Migration tools (export/import/describe)
        "firebird_export" => handle_export(cfg, args, files_base_url),
        "firebird_import" => handle_import(cfg, args, files_base_url),
        "firebird_describe_file" => handle_describe_file(cfg, args, files_base_url),

        // Data quality tools
        "firebird_data_quality" => handle_data_quality(cfg, args),

        // File tools
        "firebird_list_files" => handle_list_files(cfg, args, files_base_url),

        other => Err(format!("Unknown tool: {other}")),
    }
}
