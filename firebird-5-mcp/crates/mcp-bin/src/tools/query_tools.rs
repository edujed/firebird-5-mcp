//! Query-related tool handlers.

use firebird_5_mcp_core::types::is_valid_identifier;
use firebird_5_mcp_core::{Config, FirebirdClient, explain, query};
use serde_json::Value;

use super::common::*;

/// Executes a SQL statement (SELECT, INSERT, UPDATE, DELETE, DDL).
pub fn handle_query(cfg: &Config, args: &Value) -> Result<String, String> {
    let sql = arg_str(args, "sql")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'sql' argument is required and must be a string.".to_string())?;

    let mut client = open_client(cfg, args)?;
    let (result, message) = query::execute(&mut client, &sql).map_err(|e| e.to_string())?;

    match result {
        Some(result) => Ok(result_to_text(&result)),
        None => {
            // Opção B: anexar o plano de execução ao resultado de DML/DDL.
            match explain::explain_plan(&mut client, &sql) {
                Ok(plan) if !plan.is_empty() => Ok(format!("{message}\n\nExecution plan:\n{plan}")),
                _ => Ok(message),
            }
        }
    }
}

/// Counts rows in a table, optionally filtered by a WHERE clause.
pub fn handle_count(cfg: &Config, args: &Value) -> Result<String, String> {
    let table = arg_str(args, "table")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'table' argument is required and must be a string.".to_string())?;
    let table = table.trim().to_uppercase();
    if !is_valid_identifier(&table) {
        return Err(format!("Invalid table name: {table}"));
    }

    let where_clause = arg_str(args, "where")
        .map(|w| w.trim().to_string())
        .unwrap_or_default();

    let mut client = open_client(cfg, args)?;
    let count = query::count(&mut client, &table, &where_clause).map_err(|e| e.to_string())?;

    let mut msg = format!(
        "Count: {count} row{} in {table}",
        plural_suffix(count as usize)
    );
    if !where_clause.is_empty() {
        msg.push_str(&format!(" (WHERE {where_clause})"));
    }
    Ok(msg)
}

/// Returns a sample of rows from a table.
pub fn handle_sample(cfg: &Config, args: &Value) -> Result<String, String> {
    let table = arg_str(args, "table")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'table' argument is required and must be a string.".to_string())?;
    let table = table.trim().to_uppercase();
    if !is_valid_identifier(&table) {
        return Err(format!("Invalid table name: {table}"));
    }

    let where_clause = arg_str(args, "where")
        .map(|w| w.trim().to_string())
        .unwrap_or_default();
    let limit = arg_str(args, "limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10);

    let mut client = open_client(cfg, args)?;
    let result =
        query::sample(&mut client, &table, &where_clause, limit).map_err(|e| e.to_string())?;
    Ok(result_to_text(&result))
}

/// Inserts multiple rows into a table.
pub fn handle_insert_batch(cfg: &Config, args: &Value) -> Result<String, String> {
    let table = arg_str(args, "table")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'table' argument is required and must be a string.".to_string())?;
    let table = table.trim().to_uppercase();
    if !is_valid_identifier(&table) {
        return Err(format!("Invalid table name: {table}"));
    }

    let columns_json = arg_str(args, "columns")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "The 'columns' argument is required and must be a JSON array string.".to_string()
        })?;
    let columns: Vec<String> = serde_json::from_str(&columns_json)
        .map_err(|e| format!("Failed to parse 'columns' as JSON array: {e}"))?;
    if columns.is_empty() {
        return Err("The 'columns' array must not be empty.".to_string());
    }

    let rows_json = arg_str(args, "rows")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "The 'rows' argument is required and must be a JSON array of arrays string.".to_string()
        })?;
    let rows: Vec<Vec<Value>> = serde_json::from_str(&rows_json)
        .map_err(|e| format!("Failed to parse 'rows' as JSON array of arrays: {e}"))?;
    if rows.is_empty() {
        return Err("The 'rows' array must not be empty.".to_string());
    }

    let mut client = open_client(cfg, args)?;
    let inserted =
        query::insert_batch(&mut client, &table, &columns, &rows).map_err(|e| e.to_string())?;
    Ok(format!(
        "Inserted {inserted} row{} into {table}.",
        plural_suffix(inserted)
    ))
}

/// Executes a SQL script (multiple statements) in a single transaction.
pub fn handle_run_script(cfg: &Config, args: &Value) -> Result<String, String> {
    let db_name = arg_str(args, "database")
        .ok_or_else(|| "The 'database' argument is required.".to_string())?;
    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;

    let script = if let Some(s) = arg_str(args, "script").filter(|s| !s.is_empty()) {
        s
    } else if let Some(p) = arg_str(args, "path").filter(|s| !s.is_empty()) {
        std::fs::read_to_string(&p).map_err(|e| format!("Failed to read script file: {e}"))?
    } else {
        return Err("Either 'script' or 'path' argument is required.".to_string());
    };

    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    let executed = query::run_script(&mut client, &script).map_err(|e| e.to_string())?;
    Ok(format!(
        "Script executed successfully. {executed} statements committed."
    ))
}

/// Executes a SQL statement that may contain SET TERM blocks.
pub fn handle_execute_immediate(cfg: &Config, args: &Value) -> Result<String, String> {
    let sql = arg_str(args, "sql")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'sql' argument is required and must be a string.".to_string())?;

    let mut client = open_client(cfg, args)?;
    client
        .execute_immediate(&sql)
        .map_err(|e| format!("Execution failed: {e}"))?;
    Ok("Statement executed successfully.".to_string())
}

/// Returns the execution plan for a SQL statement.
pub fn handle_explain_plan(cfg: &Config, args: &Value) -> Result<String, String> {
    let sql = arg_str(args, "sql")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'sql' argument is required and must be a string.".to_string())?;

    let mut client = open_client(cfg, args)?;
    let plan = explain::explain_plan(&mut client, &sql).map_err(|e| e.to_string())?;
    Ok(format!(
        "Execution plan (the statement was executed inside a rolled-back transaction):\n{plan}"
    ))
}
