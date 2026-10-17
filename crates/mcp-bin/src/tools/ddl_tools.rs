//! DDL tools: create database/trigger/procedure/function/sequence, alter, drop.

use firebird_5_mcp_core::types::is_valid_identifier;
use firebird_5_mcp_core::{Config, FirebirdClient, ddl, mlog};
use serde_json::Value;

use super::ToolResult;
use super::common::{arg_str, arg_u32, open_client};

pub fn handle_create_database(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let path = arg_str(args, "path")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'path' argument is required and must be a string.".to_string())?
        .trim()
        .to_string();

    if cfg.server.allowed_paths.is_empty() {
        return Err(
            "No allowed paths configured. Add 'allowed_paths' to the server config to enable database creation."
                .to_string(),
        );
    }
    if !cfg.server.is_path_allowed(&path) {
        return Err(format!(
            "Path '{}' is not within any allowed path. Allowed: {}",
            path,
            cfg.server.allowed_paths.join(", ")
        ));
    }

    let page_size = arg_u32(args, "page_size").unwrap_or(DEFAULT_PAGE_SIZE);
    if !is_valid_page_size(page_size) {
        return Err(format!(
            "Invalid page size: {page_size}. Must be a power of two between 1024 and 32768 (1K, 2K, 4K, 8K, 16K or 32K)."
        ));
    }

    mlog!("create_database: {} (page_size={page_size})", path);
    FirebirdClient::create_database(&cfg.server, &path, page_size).map_err(|e| {
        mlog!("create_database FAILED for {path}: {e}");
        e.to_string()
    })?;

    let register = args
        .get("register")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if register {
        // Derive name from filename if not provided (e.g. "/database/test-mcp.fdb" → "test-mcp")
        let name = arg_str(args, "name")
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| {
                std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("database")
                    .to_string()
            });
        let description = arg_str(args, "description")
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string());

        // Clone config, register, and save (cfg is &Config, not &mut)
        let mut cfg_clone = cfg.clone();
        cfg_clone
            .register_database(&name, &path, description.as_deref())
            .map_err(|e| format!("Database created but registration failed: {e}"))?;

        Ok(ToolResult::with_structured(
            format!(
                "Database created successfully at {path} (page size: {page_size}).\nRegistered as '{name}' in config.json."
            ),
            serde_json::json!({
                "path": path,
                "name": name,
                "page_size": page_size,
                "registered": true
            }),
        ))
    } else {
        Ok(ToolResult::text(format!(
            "Database created successfully at {path} (page size: {page_size})."
        )))
    }
}

/// Default page size for new databases when `page_size` is not specified.
const DEFAULT_PAGE_SIZE: u32 = 8192;

/// Firebird only accepts page sizes that are powers of two in the range 1K–32K.
fn is_valid_page_size(size: u32) -> bool {
    matches!(size, 1024 | 2048 | 4096 | 8192 | 16384 | 32768)
}

pub fn handle_create_trigger(cfg: &Config, args: &Value) -> Result<String, String> {
    let db_name = arg_str(args, "database")
        .ok_or_else(|| "The 'database' argument is required.".to_string())?;
    let trigger_name = arg_str(args, "trigger_name")
        .ok_or_else(|| "The 'trigger_name' argument is required.".to_string())?;
    let table_name = arg_str(args, "table_name")
        .ok_or_else(|| "The 'table_name' argument is required.".to_string())?;
    let trigger_type = arg_str(args, "trigger_type")
        .ok_or_else(|| "The 'trigger_type' argument is required.".to_string())?;
    let timing =
        arg_str(args, "timing").ok_or_else(|| "The 'timing' argument is required.".to_string())?;
    let body =
        arg_str(args, "body").ok_or_else(|| "The 'body' argument is required.".to_string())?;

    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    ddl::create_trigger(
        &mut client,
        &trigger_name,
        &table_name,
        &trigger_type,
        &timing,
        &body,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "Trigger {trigger_name} created on table {table_name}."
    ))
}

pub fn handle_create_procedure(cfg: &Config, args: &Value) -> Result<String, String> {
    let db_name = arg_str(args, "database")
        .ok_or_else(|| "The 'database' argument is required.".to_string())?;
    let procedure_name = arg_str(args, "procedure_name")
        .ok_or_else(|| "The 'procedure_name' argument is required.".to_string())?;
    let params = arg_str(args, "params").unwrap_or_default();
    let body =
        arg_str(args, "body").ok_or_else(|| "The 'body' argument is required.".to_string())?;

    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    ddl::create_procedure(&mut client, &procedure_name, &params, &body)
        .map_err(|e| e.to_string())?;
    Ok(format!("Procedure {procedure_name} created."))
}

pub fn handle_create_function(cfg: &Config, args: &Value) -> Result<String, String> {
    let db_name = arg_str(args, "database")
        .ok_or_else(|| "The 'database' argument is required.".to_string())?;
    let function_name = arg_str(args, "function_name")
        .ok_or_else(|| "The 'function_name' argument is required.".to_string())?;
    let params = arg_str(args, "params").unwrap_or_default();
    let return_type = arg_str(args, "return_type")
        .ok_or_else(|| "The 'return_type' argument is required.".to_string())?;
    let body =
        arg_str(args, "body").ok_or_else(|| "The 'body' argument is required.".to_string())?;

    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    ddl::create_function(&mut client, &function_name, &params, &return_type, &body)
        .map_err(|e| e.to_string())?;
    Ok(format!("Function {function_name} created."))
}

pub fn handle_create_sequence(cfg: &Config, args: &Value) -> Result<String, String> {
    let db_name = arg_str(args, "database")
        .ok_or_else(|| "The 'database' argument is required.".to_string())?;
    let sequence_name = arg_str(args, "sequence_name")
        .ok_or_else(|| "The 'sequence_name' argument is required.".to_string())?;

    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    ddl::create_sequence(&mut client, &sequence_name).map_err(|e| e.to_string())?;
    Ok(format!("Sequence {sequence_name} created."))
}

pub fn handle_alter_column_type(cfg: &Config, args: &Value) -> Result<String, String> {
    let table = arg_str(args, "table").unwrap_or_default();
    let column = arg_str(args, "column").unwrap_or_default();
    let domain = arg_str(args, "domain").unwrap_or_default();
    if table.is_empty() || column.is_empty() || domain.is_empty() {
        return Err("table, column, and domain are required".to_string());
    }

    let mut client = open_client(cfg, args)?;
    ddl::alter_column_type(&mut client, &table, &column, &domain).map_err(|e| e.to_string())?;
    Ok(format!("Column {table}.{column} type changed to {domain}."))
}

pub fn handle_drop(cfg: &Config, args: &Value) -> Result<String, String> {
    let obj_type = arg_str(args, "type").unwrap_or_default();
    let name = arg_str(args, "name").unwrap_or_default();
    if obj_type.is_empty() || name.is_empty() {
        return Err("Both 'type' and 'name' arguments are required.".to_string());
    }
    let obj_type = obj_type.trim().to_uppercase();
    let name = name.trim().to_string();

    let valid = [
        "TABLE",
        "VIEW",
        "DOMAIN",
        "TRIGGER",
        "PROCEDURE",
        "FUNCTION",
        "INDEX",
        "SEQUENCE",
    ];
    if !valid.contains(&obj_type.as_str()) {
        return Err(format!(
            "Invalid type: {obj_type}. Valid types: TABLE, VIEW, DOMAIN, TRIGGER, PROCEDURE, FUNCTION, INDEX, SEQUENCE"
        ));
    }
    if !is_valid_identifier(&name) {
        return Err(format!("Invalid name: {name}"));
    }

    let mut client = open_client(cfg, args)?;
    ddl::drop(&mut client, &obj_type, &name).map_err(|e| e.to_string())?;
    Ok(format!("Dropped {obj_type} {name}."))
}
