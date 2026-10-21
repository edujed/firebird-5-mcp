//! Query-related tool handlers.

use firebird_5_mcp_core::types::{is_valid_identifier, value_to_json};
use firebird_5_mcp_core::{Config, FirebirdClient, IsolationPreset, explain, query};
use serde_json::{Value, json};

use super::ToolResult;
use super::common::*;

/// Executes a SQL statement (SELECT, INSERT, UPDATE, DELETE, DDL).
///
/// For SELECT statements, results are bounded to 1000 rows by default.
/// Use `max_rows` and `skip` to paginate large result sets.
///
/// Returns `structuredContent` with `{columns, rows, row_count, truncated}`
/// for SELECT results, so the Web UI can render a grid without parsing
/// Markdown.
pub fn handle_query(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let sql = arg_str(args, "sql")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'sql' argument is required and must be a string.".to_string())?;

    let max_rows = arg_u32(args, "max_rows")
        .unwrap_or(query::DEFAULT_ROWS)
        .min(query::MAX_ROWS);
    let skip = arg_u32(args, "skip").unwrap_or(0);

    let mut client = open_client(cfg, args)?;

    // For SELECT/WITH, use the bounded query path.
    // For everything else, fall back to the execute path.
    let is_select = query::accepts_row_limit(&sql);
    let (result, message) = if is_select {
        match query::query_with_limit(&mut client, &sql, max_rows, skip) {
            Ok(r) => (Some(r), String::new()),
            Err(e) => return Err(e.to_string()),
        }
    } else {
        query::execute(&mut client, &sql).map_err(|e| e.to_string())?
    };

    match result {
        Some(result) => {
            let mut text = result_to_text(&result);
            let truncated = result.rows.len() as u32 >= max_rows;
            // Hint for pagination when the result is full.
            if truncated {
                let next_skip = skip + max_rows;
                text.push_str(&format!(
                    "\n\n---\nResults truncated at {max_rows} rows. Use `skip: \"{next_skip}\"` to fetch the next page."
                ));
            }
            // Structured content for the Web UI grid.
            let columns: Vec<Value> = result
                .columns
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "type": c.type_name,
                        "is_numeric": c.is_numeric,
                    })
                })
                .collect();
            let rows: Vec<Value> = result
                .rows
                .iter()
                .map(|row| Value::Array(row.iter().map(value_to_json).collect()))
                .collect();
            let structured = json!({
                "columns": columns,
                "rows": rows,
                "row_count": result.rows.len(),
                "truncated": truncated,
            });
            Ok(ToolResult::with_structured(text, structured))
        }
        None => {
            // Opção B: anexar o plano de execução ao resultado de DML/DDL.
            match explain::explain_plan(&mut client, &sql) {
                Ok(plan) if !plan.is_empty() => Ok(ToolResult::text(format!(
                    "{message}\n\nExecution plan:\n{plan}"
                ))),
                _ => Ok(ToolResult::text(message)),
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
    let limit = arg_u32(args, "limit").map(|l| l as usize).unwrap_or(10);

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
        // Security: a script file path is only honored when it lies inside one
        // of the configured `allowed_paths` (same containment rule used by
        // firebird_create_database). This prevents the server from being used
        // to read arbitrary files off the host.
        if !cfg.server.is_path_allowed(&p) {
            return Err(format!(
                "Script path '{}' is not within any allowed path. Allowed: {}",
                p,
                cfg.server.allowed_paths.join(", ")
            ));
        }
        std::fs::read_to_string(&p).map_err(|e| format!("Failed to read script file: {e}"))?
    } else {
        return Err("Either 'script' or 'path' argument is required.".to_string());
    };

    let preset = match arg_str(args, "isolation") {
        Some(s) => IsolationPreset::parse(&s).ok_or_else(|| {
            format!(
                "Invalid isolation level: '{s}'. Valid values: read-committed, snapshot, consistency, readonly."
            )
        })?,
        None => IsolationPreset::default(),
    };
    let lock_wait = arg_u32(args, "lock_wait");

    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;
    let executed =
        query::run_script(&mut client, &script, preset, lock_wait).map_err(|e| e.to_string())?;

    let mut msg = format!("Script executed successfully. {executed} statements committed.");
    if preset != IsolationPreset::default() {
        msg.push_str(&format!(" (isolation: {})", preset.label()));
    }
    Ok(msg)
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
///
/// Returns `structuredContent` with `{plan}` so the Web UI can display the
/// plan without parsing Markdown.
pub fn handle_explain_plan(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let sql = arg_str(args, "sql")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'sql' argument is required and must be a string.".to_string())?;

    let mut client = open_client(cfg, args)?;
    let plan = explain::explain_plan(&mut client, &sql).map_err(|e| e.to_string())?;
    let text = format!(
        "Execution plan (the statement was executed inside a rolled-back transaction):\n{plan}"
    );
    Ok(ToolResult::with_structured(text, json!({ "plan": plan })))
}
