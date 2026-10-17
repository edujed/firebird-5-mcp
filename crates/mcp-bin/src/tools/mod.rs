//! Tool handlers for the MCP server.
//!
//! Each tool is implemented as a separate function that takes the config and
//! JSON arguments, and returns a Markdown-formatted result or an error.

use firebird_5_mcp_core::{Config, mlog};
use serde_json::Value;

mod common;
mod ddl_tools;
mod definitions;
mod generator_tools;
mod introspection_tools;
mod query_tools;
mod schema_tools;
mod stats_tools;
mod view_dto_tools;

pub use ddl_tools::*;
pub use definitions::*;
pub use generator_tools::*;
pub use introspection_tools::*;
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
pub fn dispatch(cfg: &Config, name: &str, args: &Value) -> Result<ToolResult, String> {
    mlog!("tool call: {name}");
    let result = dispatch_inner(cfg, name, args);
    if let Err(ref e) = result {
        mlog!("tool {name} ERROR: {e}");
    }
    result
}

fn dispatch_inner(cfg: &Config, name: &str, args: &Value) -> Result<ToolResult, String> {
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
        "firebird_metadata_extract" => wrap_text(handle_metadata_extract(cfg, args)),

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
        "firebird_generate_dev" => handle_generate_dev(cfg, args),
        "firebird_generate_view_dto" => handle_generate_view_dto(cfg, args),

        // DDL tools
        "firebird_create_database" => handle_create_database(cfg, args),
        "firebird_create_trigger" => wrap_text(handle_create_trigger(cfg, args)),
        "firebird_create_procedure" => wrap_text(handle_create_procedure(cfg, args)),
        "firebird_create_function" => wrap_text(handle_create_function(cfg, args)),
        "firebird_create_sequence" => wrap_text(handle_create_sequence(cfg, args)),
        "firebird_alter_column_type" => wrap_text(handle_alter_column_type(cfg, args)),
        "firebird_drop" => wrap_text(handle_drop(cfg, args)),

        other => Err(format!("Unknown tool: {other}")),
    }
}
