//! Tool handlers for the MCP server.
//!
//! Each tool is implemented as a separate function that takes the config and
//! JSON arguments, and returns a Markdown-formatted result or an error.

use firebird_5_mcp_core::Config;
use serde_json::Value;

mod common;
mod ddl_tools;
mod definitions;
mod generator_tools;
mod query_tools;
mod schema_tools;
mod stats_tools;
mod view_dto_tools;

pub use ddl_tools::*;
pub use definitions::*;
pub use generator_tools::*;
pub use query_tools::*;
pub use schema_tools::*;
pub use stats_tools::*;
pub use view_dto_tools::*;

/// Dispatches a tool call to the appropriate handler.
pub fn dispatch(cfg: &Config, name: &str, args: &Value) -> Result<String, String> {
    match name {
        // Query tools
        "firebird_query" => handle_query(cfg, args),
        "firebird_count" => handle_count(cfg, args),
        "firebird_sample" => handle_sample(cfg, args),
        "firebird_insert_batch" => handle_insert_batch(cfg, args),
        "firebird_run_script" => handle_run_script(cfg, args),
        "firebird_execute_immediate" => handle_execute_immediate(cfg, args),
        "firebird_explain_plan" => handle_explain_plan(cfg, args),

        // Schema tools
        "firebird_describe_table" => handle_describe_table(cfg, args),
        "firebird_describe_database" => handle_describe_database(cfg, args),
        "firebird_describe" => handle_describe(cfg, args),
        "firebird_list" => handle_list(cfg, args),
        "firebird_find_column" => handle_find_column(cfg, args),
        "firebird_check_duplicates" => handle_check_duplicates(cfg, args),
        "firebird_domains" => handle_domains(cfg, args),
        "firebird_get_databases" => handle_get_databases(cfg),
        "firebird_metadata_extract" => handle_metadata_extract(cfg, args),

        // Stats tools
        "firebird_crypt_state" => handle_crypt_state(cfg, args),
        "firebird_database_stats" => handle_database_stats(cfg, args),
        "firebird_query_history" => handle_query_history(cfg, args),

        // Generator tools
        "firebird_generate_dev" => handle_generate_dev(cfg, args),
        "firebird_generate_view_dto" => handle_generate_view_dto(cfg, args),

        // DDL tools
        "firebird_create_database" => handle_create_database(cfg, args),
        "firebird_create_trigger" => handle_create_trigger(cfg, args),
        "firebird_create_procedure" => handle_create_procedure(cfg, args),
        "firebird_create_function" => handle_create_function(cfg, args),
        "firebird_create_sequence" => handle_create_sequence(cfg, args),
        "firebird_alter_column_type" => handle_alter_column_type(cfg, args),
        "firebird_drop" => handle_drop(cfg, args),

        other => Err(format!("Unknown tool: {other}")),
    }
}
