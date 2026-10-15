//! DDL tools: create database/trigger/procedure/function/sequence, alter, drop.

use firebird_5_mcp_core::types::is_valid_identifier;
use firebird_5_mcp_core::{Config, FirebirdClient, ddl};
use serde_json::Value;

use super::common::{arg_str, open_client};

pub fn handle_create_database(cfg: &Config, args: &Value) -> Result<String, String> {
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

    FirebirdClient::create_database(&cfg.server, &path).map_err(|e| e.to_string())?;
    Ok(format!(
        "Database created successfully at {path} (page size: 4096)."
    ))
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
