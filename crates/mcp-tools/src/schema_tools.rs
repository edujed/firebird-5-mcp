//! Schema inspection tools: describe table/database, list objects, extract DDL.

use firebird_5_mcp_core::types::{format_column_type, is_valid_identifier};
use firebird_5_mcp_core::{Config, FirebirdClient, metadata, query, schema};
use firebird_5_mcp_types::{is_not_null, trigger_type_label};
use serde_json::{Value, json};

use super::ToolResult;
use super::common::{
    arg_bool, arg_str, cell_int, cell_text, cell_text_or, open_client, save_or_inline,
};

/// Formats a table schema as Markdown (with a table header).
pub fn format_schema(schema: &schema::TableSchema) -> String {
    format!(
        "### Table: {}\n\n{}",
        schema.table,
        schema::format_schema_markdown(schema)
    )
}

pub fn handle_describe_table(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let table_name = arg_str(args, "table_name")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'table_name' argument is required and must be a string.".to_string())?;
    let table_name = table_name.trim().to_uppercase();
    if !is_valid_identifier(&table_name) {
        return Err(format!("Invalid table name: {table_name}"));
    }

    let mut client = open_client(cfg, args)?;
    let schema = schema::describe_table(&mut client, &table_name).map_err(|e| e.to_string())?;
    let text = format_schema(&schema);
    let structured = schema_to_json(&schema);
    Ok(ToolResult::with_structured(text, structured))
}

/// Serializes a [`TableSchema`] to a JSON value for `structuredContent`.
fn schema_to_json(schema: &schema::TableSchema) -> Value {
    let columns: Vec<Value> = schema
        .columns
        .iter()
        .map(|c| {
            let type_str = format_column_type(c.type_num, c.sub_type, c.size, c.precision, c.scale);
            json!({
                "name": c.name,
                "type": type_str,
                "nullable": !is_not_null(c.nullable),
                "default": c.default,
                "position": c.position,
                "domain": c.domain,
            })
        })
        .collect();
    let indexes: Vec<Value> = schema
        .indexes
        .iter()
        .map(|i| {
            json!({
                "name": i.name,
                "unique": i.unique,
                "type": i.r#type,
                "columns": i.columns,
            })
        })
        .collect();
    let constraints: Vec<Value> = schema
        .constraints
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "type": c.r#type,
            })
        })
        .collect();
    let triggers: Vec<Value> = schema
        .triggers
        .iter()
        .map(|t| {
            let (timing, event) = t
                .r#type
                .map(|tt| trigger_type_label(tt))
                .unwrap_or(("BEFORE", "INSERT"));
            json!({
                "name": t.name,
                "timing": timing,
                "event": event,
                "sequence": t.sequence,
            })
        })
        .collect();
    let comments: Value = Value::Object(
        schema
            .comments
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    );
    json!({
        "table": schema.table,
        "columns": columns,
        "indexes": indexes,
        "constraints": constraints,
        "triggers": triggers,
        "comments": comments,
    })
}

pub fn handle_get_databases(_cfg: &Config) -> Result<ToolResult, String> {
    // Reload config from disk to pick up external changes
    // (e.g. databases registered by firebird_create_database in a previous session)
    let cfg = match Config::reload() {
        Ok(c) => c,
        Err(_) => return Err("Failed to reload config from disk".to_string()),
    };

    let mut b = String::new();
    b.push_str(&format!(
        "### Configured Databases ({})\n\n",
        cfg.server.databases.len()
    ));
    b.push_str("NAME | DESCRIPTION | PATH | DEFAULT\n");
    b.push_str("--- | --- | --- | ---\n");
    let default = cfg.default_database();
    for db in &cfg.server.databases {
        let default_mark = if db.name == default { "YES" } else { "NO" };
        let desc = db
            .description
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("-");
        b.push_str(&format!(
            "{} | {} | {} | {}\n",
            db.name, desc, db.path, default_mark
        ));
    }
    b.push_str(&format!("\n**MCP Build Time:** {}\n", crate::build_time()));

    // Structured content for the Web UI database selector.
    let databases: Vec<Value> = cfg
        .server
        .databases
        .iter()
        .map(|db| {
            json!({
                "name": db.name,
                "description": db.description,
                "path": db.path,
                "is_default": db.name == default,
            })
        })
        .collect();
    let structured = json!({
        "databases": databases,
        "default": default,
        "build_time": crate::build_time(),
    });
    Ok(ToolResult::with_structured(b, structured))
}

/// Collects the database health info from MON$DATABASE + MON$ATTACHMENTS.
/// Returns `None` if the query fails (e.g. no permission).
fn collect_db_health(client: &mut FirebirdClient) -> Option<serde_json::Value> {
    let health_sql = r#"
        SELECT
            MON$DATABASE_NAME,
            MON$ODS_MAJOR || '.' || MON$ODS_MINOR,
            MON$PAGE_SIZE,
            MON$OLDEST_TRANSACTION,
            MON$OLDEST_ACTIVE,
            MON$NEXT_TRANSACTION,
            MON$PAGE_BUFFERS,
            MON$SQL_DIALECT,
            MON$SHUTDOWN_MODE,
            MON$READ_ONLY,
            MON$FORCED_WRITES,
            MON$PAGES,
            MON$BACKUP_STATE
        FROM MON$DATABASE
    "#;
    let health_result = query::query(client, health_sql).ok()?;
    let health = health_result.rows.first()?;
    let path = cell_text(health, 0);
    let ods = cell_text(health, 1);
    let page_size = cell_int(health, 2);
    let oldest_txn = cell_int(health, 3);
    let oldest_active = cell_int(health, 4);
    let next_txn = cell_int(health, 5);
    let page_buffers = cell_int(health, 6);
    let dialect = cell_int(health, 7);
    let shutdown_mode = cell_int(health, 8);
    let read_only = cell_int(health, 9);
    let forced_writes = cell_int(health, 10);
    let pages = cell_int(health, 11);
    let backup_state = cell_int(health, 12);

    let shutdown_str = match shutdown_mode {
        0 => "Online (Normal)",
        1 => "Multi-user Shutdown (SYSDBA/Owner only, unlimited connections)",
        2 => "Single-user Shutdown (SYSDBA/Owner only, 1 connection)",
        3 => "Full Shutdown (no connections accepted)",
        _ => "Unknown",
    };
    let size_mb = (pages as f64 * page_size as f64) / (1024.0 * 1024.0);

    let conn_sql = r#"
        SELECT COUNT(*) FROM MON$ATTACHMENTS
        WHERE MON$STATE = 1 AND MON$SYSTEM_FLAG = 0
    "#;
    let active_conns = query::query(client, conn_sql)
        .ok()
        .and_then(|r| r.rows.first().map(|r| cell_int(r, 0)))
        .unwrap_or(0);

    Some(serde_json::json!({
        "path": path,
        "ods": ods,
        "page_size": page_size,
        "pages": pages,
        "size_mb": size_mb,
        "dialect": dialect,
        "mode": shutdown_str,
        "read_only": read_only == 1,
        "forced_writes": forced_writes == 1,
        "active_connections": active_conns,
        "oldest_transaction": oldest_txn,
        "oldest_active": oldest_active,
        "next_transaction": next_txn,
        "buffer_pool": page_buffers,
        "backup_state": backup_state,
    }))
}

/// Collects the full schema of a single table (columns, PKs, comments, indexes).
fn collect_table_schema(
    client: &mut FirebirdClient,
    table_name: &str,
    include_counts: bool,
) -> Option<serde_json::Value> {
    let ts = schema::describe_table(client, table_name).ok()?;

    // PK columns from indexes named RDB$PRIMARY*
    let mut pk_columns: std::collections::HashSet<String> = std::collections::HashSet::new();
    for idx in &ts.indexes {
        if idx.name.starts_with("RDB$PRIMARY") {
            for col in idx.columns.split(", ") {
                pk_columns.insert(col.trim().to_string());
            }
        }
    }

    let columns: Vec<serde_json::Value> = ts
        .columns
        .iter()
        .map(|col| {
            let type_str = format_column_type(
                col.type_num,
                col.sub_type,
                col.size,
                col.precision,
                col.scale,
            );
            let mut obj = serde_json::json!({
                "name": col.name,
                "type": type_str,
                "nullable": col.nullable.map(|n| n == 1).unwrap_or(true),
            });
            if pk_columns.contains(&col.name) {
                obj["pk"] = serde_json::json!(true);
            }
            if let Some(comment) = ts.comments.get(&col.name) {
                if !comment.is_empty() {
                    obj["comment"] = serde_json::json!(comment);
                }
            }
            obj
        })
        .collect();

    let mut obj = serde_json::json!({
        "name": table_name,
        "columns": columns,
    });
    if let Some(comment) = ts.comments.get("table") {
        if !comment.is_empty() {
            obj["comment"] = serde_json::json!(comment);
        }
    }
    if include_counts {
        if let Ok(count) = query::count(client, table_name, "") {
            obj["row_count"] = serde_json::json!(count);
        }
    }
    Some(obj)
}

/// Renders the Markdown output for `firebird_describe_database`.
fn render_describe_database_markdown(
    db_name: &str,
    health: &Option<serde_json::Value>,
    tables: &[serde_json::Value],
    mmd: &str,
) -> String {
    let mut b = String::new();
    b.push_str(&format!("### Database: {db_name}\n\n"));

    if let Some(h) = health {
        b.push_str("#### Health\n\n");
        b.push_str(&format!(
            "- **Path:** {}\n",
            h["path"].as_str().unwrap_or("?")
        ));
        b.push_str(&format!(
            "- **ODS Version:** {}\n",
            h["ods"].as_str().unwrap_or("?")
        ));
        b.push_str(&format!(
            "- **Page Size:** {} bytes\n",
            h["page_size"].as_i64().unwrap_or(0)
        ));
        b.push_str(&format!(
            "- **File Size:** {} pages (~{:.1} MB)\n",
            h["pages"].as_i64().unwrap_or(0),
            h["size_mb"].as_f64().unwrap_or(0.0)
        ));
        b.push_str(&format!(
            "- **Dialect:** {}\n",
            h["dialect"].as_i64().unwrap_or(0)
        ));
        b.push_str(&format!(
            "- **Mode:** {}\n",
            h["mode"].as_str().unwrap_or("?")
        ));
        b.push_str(&format!(
            "- **Read-Only:** {}\n",
            if h["read_only"].as_bool().unwrap_or(false) {
                "YES"
            } else {
                "NO"
            }
        ));
        b.push_str(&format!(
            "- **Forced Writes:** {}\n",
            if h["forced_writes"].as_bool().unwrap_or(false) {
                "YES"
            } else {
                "NO"
            }
        ));
        b.push_str(&format!(
            "- **Active Connections:** {}\n",
            h["active_connections"].as_i64().unwrap_or(0)
        ));
        b.push_str(&format!(
            "- **Oldest Transaction:** {} (active: {}, next: {})\n",
            h["oldest_transaction"].as_i64().unwrap_or(0),
            h["oldest_active"].as_i64().unwrap_or(0),
            h["next_transaction"].as_i64().unwrap_or(0)
        ));
        b.push_str(&format!(
            "- **Buffer Pool:** {} pages\n",
            h["buffer_pool"].as_i64().unwrap_or(0)
        ));
        if h["backup_state"].as_i64().unwrap_or(0) != 0 {
            b.push_str(&format!(
                "- **Backup:** IN PROGRESS (state {})\n",
                h["backup_state"].as_i64().unwrap_or(0)
            ));
        }
        b.push('\n');
    }

    b.push_str(&format!("#### Tables ({})\n\n", tables.len()));
    for t in tables {
        b.push_str(&format!("**{}**", t["name"].as_str().unwrap_or("?")));
        if let Some(count) = t.get("row_count").and_then(|v| v.as_i64()) {
            b.push_str(&format!(" ({count} rows)"));
        }
        b.push('\n');
        if let Some(comment) = t.get("comment").and_then(|v| v.as_str()) {
            b.push_str(&format!("  // {comment}\n"));
        }
        if let Some(cols) = t.get("columns").and_then(|v| v.as_array()) {
            for col in cols {
                let mut line = format!(
                    "  - {} ({})",
                    col["name"].as_str().unwrap_or("?"),
                    col["type"].as_str().unwrap_or("?")
                );
                if col.get("pk").and_then(|v| v.as_bool()).unwrap_or(false) {
                    line.push_str(", PK");
                }
                if !col
                    .get("nullable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
                {
                    line.push_str(", NOT NULL");
                }
                if let Some(c) = col.get("comment").and_then(|v| v.as_str()) {
                    line.push_str(&format!(" // {c}"));
                }
                b.push_str(&line);
                b.push('\n');
            }
        }
        b.push('\n');
    }

    b.push_str("#### Relationships (Mermaid)\n");
    b.push_str("```mermaid\n");
    b.push_str(mmd);
    b.push_str("```");
    b.push('\n');
    b
}

pub fn handle_describe_database(
    cfg: &Config,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let include_counts = arg_bool(args, "include_counts").unwrap_or(false);
    let include_describes = arg_bool(args, "include_describes").unwrap_or(true);
    let output_mode = arg_str(args, "output_mode").unwrap_or_else(|| "inline".to_string());
    let output_format = arg_str(args, "output_format").unwrap_or_else(|| "markdown".to_string());
    let compression = arg_str(args, "compression").unwrap_or_default();
    let has_output_dir = arg_str(args, "outputDirPath")
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    // Validate output_mode
    let output_mode = output_mode.to_lowercase();
    if !matches!(output_mode.as_str(), "inline" | "download") {
        return Err(format!(
            "invalid output_mode '{output_mode}' — valid: 'inline', 'download'"
        ));
    }

    // Validate output_format
    let output_format = output_format.to_lowercase();
    if !matches!(output_format.as_str(), "markdown" | "json") {
        return Err(format!(
            "invalid output_format '{output_format}' — valid: 'markdown', 'json'"
        ));
    }

    // Validate compression
    if !compression.is_empty() && compression != "none" {
        match compression.to_lowercase().as_str() {
            "zstd" | "gzip" | "lz4" => {}
            other => {
                return Err(format!(
                    "unsupported compression '{other}' — valid: zstd, gzip, lz4, none"
                ));
            }
        }
    }

    // If output_mode is "download" but no outputDirPath, use the default storage dir
    let effective_output_dir = if has_output_dir {
        Some(arg_str(args, "outputDirPath").unwrap())
    } else if output_mode == "download" {
        // Use the default storage dir (resolved by save_or_inline)
        None // save_or_inline will resolve it
    } else {
        None
    };

    let db = cfg
        .server
        .find_database(&db_name)
        .map_err(|e| e.to_string())?;
    let mut client = FirebirdClient::new(&cfg.server, &db.path).map_err(|e| e.to_string())?;

    // Collect health
    let health = collect_db_health(&mut client);

    // Collect tables
    let tables_list = schema::list_tables(&mut client).map_err(|e| e.to_string())?;
    let fks = schema::get_foreign_keys(&mut client).map_err(|e| e.to_string())?;

    let mut table_values: Vec<serde_json::Value> = Vec::new();
    for table in &tables_list {
        if table.r#type != "TABLE" {
            continue;
        }
        if include_describes {
            if let Some(tv) = collect_table_schema(&mut client, &table.name, include_counts) {
                table_values.push(tv);
            }
        } else {
            table_values.push(serde_json::json!({
                "name": table.name,
                "columns": [],
            }));
        }
    }

    // Build the Mermaid erDiagram (shared with handle_er_diagram)
    let table_infos: Vec<schema::TableInfo> = tables_list
        .iter()
        .filter(|t| t.r#type == "TABLE")
        .cloned()
        .collect();
    let mmd = build_er_diagram(&mut client, &table_infos, &fks, false);

    // Determine output
    let should_save = output_mode == "download" || has_output_dir;

    if should_save {
        // Build the content (Markdown or JSON)
        let content = if output_format == "json" {
            let json_obj = serde_json::json!({
                "database": db_name,
                "health": health,
                "tables": table_values,
                "er_diagram": mmd,
            });
            serde_json::to_string_pretty(&json_obj).map_err(|e| e.to_string())?
        } else {
            render_describe_database_markdown(&db_name, &health, &table_values, &mmd)
        };

        // Determine file name and extension
        let mut name = format!("{}_describe", db_name);
        let ext = if output_format == "json" {
            "json"
        } else {
            "md"
        };
        name.push_str(&format!(".{ext}"));
        if !compression.is_empty() && compression != "none" {
            let suffix = match compression.to_lowercase().as_str() {
                "zstd" => ".zst",
                "gzip" => ".gz",
                "lz4" => ".lz4",
                _ => unreachable!(),
            };
            name.push_str(suffix);
        }

        let files = vec![firebird_5_mcp_core::codegen::targets::GeneratedFile {
            name,
            content,
            format: ext.to_string(),
        }];

        // If outputDirPath was explicitly provided, use it; otherwise let save_or_inline resolve the default
        let args_for_save = if effective_output_dir.is_some() {
            args.clone()
        } else {
            // Remove outputDirPath from args so save_or_inline uses the default
            let mut new_args = args.clone();
            new_args.as_object_mut().map(|o| o.remove("outputDirPath"));
            new_args
        };

        save_or_inline(cfg, &args_for_save, &db_name, ext, &files, files_base_url)
    } else {
        // Inline output
        let text = if output_format == "json" {
            let json_obj = serde_json::json!({
                "database": db_name,
                "health": health,
                "tables": table_values,
                "er_diagram": mmd,
            });
            serde_json::to_string_pretty(&json_obj).map_err(|e| e.to_string())?
        } else {
            render_describe_database_markdown(&db_name, &health, &table_values, &mmd)
        };
        Ok(ToolResult::text(text))
    }
}

pub fn handle_list(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let obj_type =
        arg_str(args, "type").ok_or_else(|| "The 'type' argument is required.".to_string())?;
    let obj_type = obj_type.trim().to_uppercase();

    let valid = [
        "TABLES",
        "PROCEDURES",
        "FUNCTIONS",
        "DOMAINS",
        "TRIGGERS",
        "INDEXES",
        "SEQUENCES",
    ];
    if !valid.contains(&obj_type.as_str()) {
        return Err(format!(
            "Invalid type: {obj_type}. Valid types: TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, SEQUENCES"
        ));
    }

    let mut client = open_client(cfg, args)?;
    let mut b = String::new();
    let mut objects: Vec<Value> = Vec::new();

    match obj_type.as_str() {
        "TABLES" => {
            let tables = schema::list_tables(&mut client).map_err(|e| e.to_string())?;
            b.push_str(&format!("### Tables and Views ({})\n\n", tables.len()));
            if tables.is_empty() {
                b.push_str("No tables or views found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str("NAME | TYPE | COMMENT\n");
            b.push_str("--- | --- | ---\n");
            for t in &tables {
                let comment = if t.comment.is_empty() {
                    "-"
                } else {
                    t.comment.as_str()
                };
                b.push_str(&format!("{} | {} | {comment}\n", t.name, t.r#type));
                objects.push(json!({
                    "name": t.name,
                    "type": t.r#type,
                    "comment": t.comment,
                }));
            }
        }
        "PROCEDURES" => {
            let sql = r#"
                SELECT
                    RDB$PROCEDURE_NAME,
                    RDB$PROCEDURE_INPUTS,
                    RDB$PROCEDURE_OUTPUTS,
                    RDB$DESCRIPTION
                FROM RDB$PROCEDURES
                ORDER BY RDB$PROCEDURE_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### Stored Procedures\n\n");
            if result.rows.is_empty() {
                b.push_str("No stored procedures found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} procedures\n\n", result.rows.len()));
            b.push_str("NAME | PARAMETERS | RETURNS | DESCRIPTION\n");
            b.push_str("--- | --- | --- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let params = cell_int(row, 1);
                let returns = cell_int(row, 2);
                let desc = cell_text_or(row, 3, "-");
                b.push_str(&format!("{name} | {params} | {returns} | {desc}\n"));
                objects.push(json!({
                    "name": name,
                    "parameters": params,
                    "returns": returns,
                    "description": desc,
                }));
            }
        }
        "FUNCTIONS" => {
            let sql = r#"
                SELECT
                    RDB$FUNCTION_NAME,
                    RDB$FUNCTION_TYPE,
                    RDB$DESCRIPTION
                FROM RDB$FUNCTIONS
                ORDER BY RDB$FUNCTION_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### User-Defined Functions\n\n");
            if result.rows.is_empty() {
                b.push_str("No user-defined functions found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} functions\n\n", result.rows.len()));
            b.push_str("NAME | TYPE | DESCRIPTION\n");
            b.push_str("--- | --- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let fn_type = cell_int(row, 1);
                let desc = cell_text_or(row, 2, "-");
                let type_name = match fn_type {
                    0 => "SQL",
                    1 => "PASM",
                    _ => "-",
                };
                b.push_str(&format!("{name} | {type_name} | {desc}\n"));
                objects.push(json!({
                    "name": name,
                    "type": type_name,
                    "description": desc,
                }));
            }
        }
        "DOMAINS" => {
            let sql = r#"
                SELECT
                    RDB$FIELD_NAME,
                    RDB$FIELD_TYPE,
                    RDB$FIELD_SUB_TYPE,
                    RDB$FIELD_LENGTH,
                    RDB$FIELD_PRECISION,
                    RDB$FIELD_SCALE,
                    RDB$DEFAULT_SOURCE,
                    RDB$NULL_FLAG,
                    RDB$DESCRIPTION
                FROM RDB$FIELDS
                WHERE RDB$SYSTEM_FLAG = 0
                  AND RDB$FIELD_NAME NOT LIKE 'RDB$%'
                  AND RDB$FIELD_NAME NOT LIKE 'MON$%'
                  AND RDB$FIELD_NAME NOT LIKE 'SEC$%'
                ORDER BY RDB$FIELD_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### Domains (Named Data Types)\n\n");
            if result.rows.is_empty() {
                b.push_str("No domains found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} domains\n\n", result.rows.len()));
            b.push_str("NAME | TYPE | DEFAULT | NULL\n");
            b.push_str("--- | --- | --- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let type_str = format_column_type(
                    cell_int(row, 1),
                    cell_int(row, 2),
                    cell_int(row, 3),
                    cell_int(row, 4),
                    cell_int(row, 5),
                );
                let def = cell_text_or(row, 6, "-");
                let null_flag = cell_int(row, 7);
                let null_str = if is_not_null(Some(null_flag)) {
                    "NO"
                } else {
                    "YES"
                };
                b.push_str(&format!("{name} | {type_str} | {def} | {null_str}\n"));
                objects.push(json!({
                    "name": name,
                    "type": type_str,
                    "default": def,
                    "nullable": null_str == "YES",
                }));
            }
        }
        "TRIGGERS" => {
            let sql = r#"
                SELECT
                    RDB$TRIGGER_NAME,
                    RDB$TRIGGER_TYPE,
                    RDB$RELATION_NAME,
                    RDB$DESCRIPTION
                FROM RDB$TRIGGERS
                WHERE RDB$SYSTEM_FLAG = 0
                ORDER BY RDB$TRIGGER_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### Triggers\n\n");
            if result.rows.is_empty() {
                b.push_str("No triggers found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} triggers\n\n", result.rows.len()));
            b.push_str("NAME | TYPE | TABLE | DESCRIPTION\n");
            b.push_str("--- | --- | --- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let trig_type = cell_int(row, 1);
                let table = cell_text(row, 2);
                let desc = cell_text_or(row, 3, "-");
                let (timing, event) = trigger_type_label(trig_type);
                let type_name = format!("{timing} {event}");
                b.push_str(&format!("{name} | {type_name} | {table} | {desc}\n"));
                objects.push(json!({
                    "name": name,
                    "type": type_name,
                    "table": table,
                    "description": desc,
                }));
            }
        }
        "INDEXES" => {
            let sql = r#"
                SELECT
                    RDB$INDEX_NAME,
                    RDB$RELATION_NAME,
                    RDB$UNIQUE_FLAG,
                    RDB$DESCRIPTION
                FROM RDB$INDICES
                WHERE RDB$SYSTEM_FLAG = 0
                ORDER BY RDB$INDEX_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### Indexes\n\n");
            if result.rows.is_empty() {
                b.push_str("No indexes found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} indexes\n\n", result.rows.len()));
            b.push_str("NAME | TABLE | UNIQUE | DESCRIPTION\n");
            b.push_str("--- | --- | --- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let table = cell_text(row, 1);
                let unique_flag = cell_int(row, 2);
                let desc = cell_text_or(row, 3, "-");
                let unique_str = if unique_flag == 1 { "YES" } else { "NO" };
                b.push_str(&format!("{name} | {table} | {unique_str} | {desc}\n"));
                objects.push(json!({
                    "name": name,
                    "table": table,
                    "unique": unique_flag == 1,
                    "description": desc,
                }));
            }
        }
        "SEQUENCES" => {
            let sql = r#"
                SELECT
                    RDB$GENERATOR_NAME,
                    RDB$DESCRIPTION
                FROM RDB$GENERATORS
                WHERE RDB$SYSTEM_FLAG = 0
                ORDER BY RDB$GENERATOR_NAME
            "#;
            let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
            b.push_str("### Sequences\n\n");
            if result.rows.is_empty() {
                b.push_str("No sequences found.\n");
                return Ok(ToolResult::with_structured(
                    b,
                    json!({ "type": obj_type, "objects": [] }),
                ));
            }
            b.push_str(&format!("Count: {} sequences\n\n", result.rows.len()));
            b.push_str("NAME | DESCRIPTION\n");
            b.push_str("--- | ---\n");
            for row in &result.rows {
                let name = cell_text(row, 0);
                let desc = cell_text_or(row, 1, "-");
                b.push_str(&format!("{name} | {desc}\n"));
                objects.push(json!({
                    "name": name,
                    "description": desc,
                }));
            }
        }
        _ => unreachable!(),
    }
    Ok(ToolResult::with_structured(
        b,
        json!({ "type": obj_type, "objects": objects }),
    ))
}

pub fn handle_metadata_extract(
    cfg: &Config,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    let mut client = open_client(cfg, args)?;
    // Check for optional 'objects' filter
    let objects: Option<Vec<String>> = args.get("objects").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });
    let ddl = match objects {
        Some(ref objs) if !objs.is_empty() => {
            metadata::extract_ddl_filtered(&mut client, Some(objs)).map_err(|e| e.to_string())?
        }
        _ => metadata::extract_ddl(&mut client).map_err(|e| e.to_string())?,
    };

    // Check if outputDirPath is provided - if so, save to file
    let db_name = arg_str(args, "database").unwrap_or_else(|| "default".to_string());
    let has_output_dir = arg_str(args, "outputDirPath")
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if has_output_dir {
        // Create a GeneratedFile with the DDL content
        let mut name = format!("{}_metadata.sql", db_name);
        if let Some(comp) = arg_str(args, "compression").filter(|s| !s.is_empty() && s != "none") {
            let suffix = match comp.to_lowercase().as_str() {
                "zstd" => ".zst",
                "gzip" => ".gz",
                "lz4" => ".lz4",
                other => {
                    return Err(format!(
                        "unsupported compression '{other}' — valid: zstd, gzip, lz4, none"
                    ));
                }
            };
            name.push_str(suffix);
        }
        let files = vec![firebird_5_mcp_core::codegen::targets::GeneratedFile {
            name,
            content: ddl,
            format: "sql".to_string(),
        }];
        save_or_inline(cfg, args, &db_name, "sql", &files, files_base_url)
    } else {
        // Return inline as before, with structured content for the Web UI.
        let structured = json!({
            "database": db_name,
            "objects": objects.unwrap_or_default(),
            "ddl": ddl,
        });
        Ok(ToolResult::with_structured(ddl, structured))
    }
}

pub fn handle_describe(cfg: &Config, args: &Value) -> Result<String, String> {
    let objects = args
        .get("objects")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "The 'objects' argument is required and must be an array of strings.".to_string()
        })?;
    let names: Vec<String> = objects
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    if names.is_empty() {
        return Err("The 'objects' array must contain at least one object name.".to_string());
    }

    let mut client = open_client(cfg, args)?;
    metadata::describe_objects(&mut client, &names).map_err(|e| e.to_string())
}

/// Builds the raw Mermaid `erDiagram` body (without the `erDiagram` header or
/// Markdown code fences) for the given tables and foreign keys.
///
/// Shared by `handle_er_diagram` (standalone tool) and
/// `handle_describe_database` (embedded in the full DB overview).
pub fn build_er_diagram(
    client: &mut FirebirdClient,
    tables: &[schema::TableInfo],
    fks: &[schema::ForeignKeyInfo],
    include_columns: bool,
) -> String {
    let mut mmd = String::from("erDiagram\n");
    for table in tables {
        mmd.push_str(&format!("    {} {{\n", table.name));
        if include_columns {
            if let Ok(ts) = schema::describe_table(client, &table.name) {
                for col in &ts.columns {
                    let type_str = format_column_type(
                        col.type_num,
                        col.sub_type,
                        col.size,
                        col.precision,
                        col.scale,
                    );
                    mmd.push_str(&format!("        {} {}\n", type_str, col.name));
                }
            }
        }
        mmd.push_str("    }\n");
    }

    // Relationships — only include FKs where both tables are in the diagram
    let table_names: std::collections::HashSet<&str> =
        tables.iter().map(|t| t.name.as_str()).collect();
    for fk in fks {
        if table_names.contains(fk.table.as_str()) && table_names.contains(fk.ref_table.as_str()) {
            mmd.push_str(&format!(
                "    {} ||--o{{ {} : \"{}\"\n",
                fk.ref_table, fk.table, fk.constraint_name
            ));
        }
    }
    mmd
}

/// Generates a Mermaid erDiagram for the database (or a subset of tables).
///
/// Returns the diagram as a Markdown code block (```` ```mermaid ````) so it
/// renders in any Markdown viewer. When `outputDirPath` is provided, the
/// diagram is also saved to a `.mmd` file (with optional compression) and a
/// `download_url` is included in SSE mode.
pub fn handle_er_diagram(
    cfg: &Config,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let include_columns = arg_bool(args, "include_columns").unwrap_or(false);
    let compression = arg_str(args, "compression").unwrap_or_default();
    let has_output_dir = arg_str(args, "outputDirPath")
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    // Optional table filter
    let tables_filter: Option<Vec<String>> =
        args.get("tables").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_uppercase()))
                .filter(|s| !s.is_empty())
                .collect()
        });

    let mut client = open_client(cfg, args)?;
    let all_tables = schema::list_tables(&mut client).map_err(|e| e.to_string())?;
    let fks = schema::get_foreign_keys(&mut client).map_err(|e| e.to_string())?;

    // Determine which tables to include
    let tables: Vec<schema::TableInfo> = match &tables_filter {
        Some(filter) if !filter.is_empty() => {
            let filter_set: std::collections::HashSet<&str> =
                filter.iter().map(|s| s.as_str()).collect();
            all_tables
                .into_iter()
                .filter(|t| t.r#type == "TABLE" && filter_set.contains(t.name.as_str()))
                .collect()
        }
        _ => all_tables
            .into_iter()
            .filter(|t| t.r#type == "TABLE")
            .collect(),
    };

    if tables.is_empty() {
        return Err("No tables found (check the 'tables' filter if provided)".to_string());
    }

    // Build the Mermaid erDiagram (shared with handle_describe_database)
    let mmd = build_er_diagram(&mut client, &tables, &fks, include_columns);

    // Wrap in a Markdown code block for rendering
    let text = format!("```mermaid\n{mmd}```");

    // Optionally save to file
    if has_output_dir {
        let mut name = format!("{}_er_diagram.mmd", db_name);
        if !compression.is_empty() && compression != "none" {
            let suffix = match compression.to_lowercase().as_str() {
                "zstd" => ".zst",
                "gzip" => ".gz",
                "lz4" => ".lz4",
                other => {
                    return Err(format!(
                        "unsupported compression '{other}' — valid: zstd, gzip, lz4, none"
                    ));
                }
            };
            name.push_str(suffix);
        }
        let files = vec![firebird_5_mcp_core::codegen::targets::GeneratedFile {
            name,
            content: mmd,
            format: "mermaid".to_string(),
        }];
        save_or_inline(cfg, args, &db_name, "mermaid", &files, files_base_url)
    } else {
        Ok(ToolResult::text(text))
    }
}

/// Checks for duplicate values in one or more columns (validates uniqueness before creating index/constraint).
pub fn handle_check_duplicates(cfg: &Config, args: &Value) -> Result<String, String> {
    let table = arg_str(args, "table")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'table' argument is required.".to_string())?;
    let table = table.trim().to_uppercase();
    if !is_valid_identifier(&table) {
        return Err(format!("Invalid table name: {table}"));
    }

    let columns_raw = args
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "The 'columns' argument is required (array of column names).".to_string())?;
    let columns: Vec<String> = columns_raw
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    if columns.is_empty() {
        return Err("The 'columns' array must contain at least one column name.".to_string());
    }
    for col in &columns {
        if !is_valid_identifier(col) {
            return Err(format!("Invalid column name: {col}"));
        }
    }

    let limit = arg_str(args, "limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10);

    let mut client = open_client(cfg, args)?;

    // Step 1: Find duplicate combinations (GROUP BY + HAVING)
    let cols_list = columns.join(", ");
    let sql = format!(
        r#"SELECT FIRST {limit} {cols_list}, COUNT(*) AS DUPLICATED_COUNT
            FROM {table}
            GROUP BY {cols_list}
            HAVING COUNT(*) > 1"#
    );

    let result = query::query(&mut client, &sql).map_err(|e| e.to_string())?;

    let mut b = String::new();
    b.push_str(&format!("### Duplicate Check: {table} ({cols_list})\n\n"));

    if result.rows.is_empty() {
        b.push_str("✅ No duplicates found. Safe to create unique index/constraint.\n");
        return Ok(b);
    }

    // Step 2: Show the duplicate combinations found
    b.push_str(&format!(
        "⚠️ {} duplicate combination(s) found:\n\n",
        result.rows.len()
    ));
    let headers: Vec<String> = result.columns.iter().map(|c| c.name.clone()).collect();
    b.push_str(&headers.join(" | "));
    b.push('\n');
    b.push_str(&"---".repeat(headers.len()));
    b.push('\n');
    for row in &result.rows {
        let cells: Vec<String> = row
            .iter()
            .map(|v| firebird_5_mcp_core::types::format_cell(v))
            .collect();
        b.push_str(&cells.join(" | "));
        b.push('\n');
    }

    // Step 3: Fetch full rows for the first duplicate combination ("contorno")
    // This shows the actual duplicate records so the dev can see what to fix
    if let Some(first_dup_row) = result.rows.first() {
        // Build WHERE clause from the first duplicate combination
        let mut conditions = Vec::new();
        for (i, col) in columns.iter().enumerate() {
            let val = &first_dup_row[i];
            let val_str = firebird_5_mcp_core::types::format_cell(val);
            // Quote string values, leave numbers as-is
            let condition = if matches!(val, firebird_5_mcp_core::query::Value::Int(_)) {
                format!("{col} = {val_str}")
            } else if matches!(val, firebird_5_mcp_core::query::Value::Null) {
                format!("{col} IS NULL")
            } else {
                format!("{col} = '{val_str}'")
            };
            conditions.push(condition);
        }
        let where_clause = conditions.join(" AND ");

        b.push_str(&format!(
            "\n#### Full rows for first duplicate ({where_clause}):\n\n"
        ));

        let detail_sql = format!("SELECT * FROM {table} WHERE {where_clause}");
        if let Ok(detail_result) = query::query(&mut client, &detail_sql) {
            if !detail_result.rows.is_empty() {
                let detail_headers: Vec<String> = detail_result
                    .columns
                    .iter()
                    .map(|c| c.name.clone())
                    .collect();
                b.push_str(&detail_headers.join(" | "));
                b.push('\n');
                b.push_str(&"---".repeat(detail_headers.len()));
                b.push('\n');
                for row in &detail_result.rows {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|v| firebird_5_mcp_core::types::format_cell(v))
                        .collect();
                    b.push_str(&cells.join(" | "));
                    b.push('\n');
                }
            }
        }
    }

    b.push_str(&format!(
        "\n💡 Fix duplicates before creating the unique index/constraint.\n"
    ));

    Ok(b)
}

/// Analyzes user-defined domains: lists them with usage counts, shows the most
/// common column type signatures, and suggests domains that could be created.
pub fn handle_domains(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;

    // Query 1: user-defined domains (domains live in RDB$FIELDS with RDB$OWNER_NAME set;
    // system-generated fields like RDB$1 must be excluded).
    let q_domains = r#"
        SELECT
            TRIM(f.RDB$FIELD_NAME) AS DOMINIO,
            f.RDB$FIELD_TYPE,
            f.RDB$FIELD_SUB_TYPE,
            f.RDB$FIELD_LENGTH,
            f.RDB$FIELD_PRECISION,
            f.RDB$FIELD_SCALE,
            f.RDB$NULL_FLAG,
            f.RDB$DEFAULT_VALUE IS NOT NULL AS TEM_DEFAULT,
            TRIM(f.RDB$DESCRIPTION) AS DESCRICAO
        FROM RDB$FIELDS f
        WHERE f.RDB$OWNER_NAME IS NOT NULL
          AND f.RDB$SYSTEM_FLAG = 0
          AND f.RDB$FIELD_NAME NOT LIKE 'RDB$%'
        ORDER BY f.RDB$FIELD_NAME
    "#;

    // Query 2: usage count per domain (columns whose RDB$FIELD_SOURCE is a domain).
    let q_usage = r#"
        SELECT rf.RDB$FIELD_SOURCE, COUNT(*) AS USOS
        FROM RDB$RELATION_FIELDS rf
        JOIN RDB$RELATIONS r ON rf.RDB$RELATION_NAME = r.RDB$RELATION_NAME
        JOIN RDB$FIELDS f ON rf.RDB$FIELD_SOURCE = f.RDB$FIELD_NAME
        WHERE r.RDB$SYSTEM_FLAG = 0
          AND f.RDB$OWNER_NAME IS NOT NULL
          AND f.RDB$FIELD_NAME NOT LIKE 'RDB$%'
        GROUP BY rf.RDB$FIELD_SOURCE
    "#;

    // Query 3: most common column type signatures, with how many already use a domain.
    let q_types = r#"
        SELECT
            f.RDB$FIELD_TYPE,
            f.RDB$FIELD_SUB_TYPE,
            f.RDB$FIELD_LENGTH,
            f.RDB$FIELD_PRECISION,
            f.RDB$FIELD_SCALE,
            COUNT(*) AS COLS,
            SUM(CASE WHEN d.RDB$FIELD_NAME IS NOT NULL THEN 1 ELSE 0 END) AS COM_DOMINIO
        FROM RDB$RELATION_FIELDS rf
        JOIN RDB$RELATIONS r ON rf.RDB$RELATION_NAME = r.RDB$RELATION_NAME
        JOIN RDB$FIELDS f ON rf.RDB$FIELD_SOURCE = f.RDB$FIELD_NAME
        LEFT JOIN RDB$FIELDS d
          ON d.RDB$FIELD_NAME = rf.RDB$FIELD_SOURCE
         AND d.RDB$OWNER_NAME IS NOT NULL
         AND d.RDB$FIELD_NAME NOT LIKE 'RDB$%'
        WHERE r.RDB$SYSTEM_FLAG = 0
        GROUP BY f.RDB$FIELD_TYPE, f.RDB$FIELD_SUB_TYPE, f.RDB$FIELD_LENGTH,
                 f.RDB$FIELD_PRECISION, f.RDB$FIELD_SCALE
        ORDER BY COLS DESC
    "#;

    let domains = query::query(&mut client, q_domains).map_err(|e| e.to_string())?;
    let usage = query::query(&mut client, q_usage).map_err(|e| e.to_string())?;
    let types = query::query(&mut client, q_types).map_err(|e| e.to_string())?;

    // Usage map: domain name -> count
    let mut usage_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in &usage.rows {
        let name = cell_text(row, 0);
        let count = cell_int(row, 1);
        usage_map.insert(name, count);
    }

    let mut b = String::new();
    b.push_str(&format!(
        "### Domain Analysis ({} user domains)\n\n",
        domains.rows.len()
    ));

    // Section 1: domains with usage
    b.push_str("#### User Domains\n\n");
    b.push_str("DOMAIN | TYPE | NULL | DEFAULT | USAGE | DESCRIPTION\n");
    b.push_str("--- | --- | --- | --- | --- | ---\n");
    for row in &domains.rows {
        let name = cell_text(row, 0);
        let type_str = format_column_type(
            cell_int(row, 1),
            cell_int(row, 2),
            cell_int(row, 3),
            cell_int(row, 4),
            cell_int(row, 5),
        );
        let null_flag = cell_int(row, 6);
        let null_str = if is_not_null(Some(null_flag)) {
            "NO"
        } else {
            "YES"
        };
        let has_default = cell_int(row, 7);
        let default_str = if has_default == 1 { "YES" } else { "NO" };
        let uses = usage_map.get(&name).copied().unwrap_or(0);
        let desc = cell_text_or(row, 8, "-");
        b.push_str(&format!(
            "{name} | {type_str} | {null_str} | {default_str} | {uses} | {desc}\n"
        ));
    }

    // Section 2: most common column types
    b.push_str("\n#### Most Common Column Types\n\n");
    b.push_str("TYPE | COLUMNS | WITH DOMAIN | WITHOUT DOMAIN\n");
    b.push_str("--- | --- | --- | ---\n");
    let mut suggestions: Vec<(String, i64)> = Vec::new();
    for row in &types.rows {
        let type_str = format_column_type(
            cell_int(row, 0),
            cell_int(row, 1),
            cell_int(row, 2),
            cell_int(row, 3),
            cell_int(row, 4),
        );
        let cols = cell_int(row, 5);
        let with_domain = cell_int(row, 6);
        let without = cols - with_domain;
        b.push_str(&format!(
            "{type_str} | {cols} | {with_domain} | {without}\n"
        ));
        // Suggest a domain for common types (>= 3 columns) that lack one
        if without >= 3 {
            suggestions.push((type_str, without));
        }
    }

    // Section 3: suggestions
    if suggestions.is_empty() {
        b.push_str("\n#### Suggestions\n\n");
        b.push_str("✅ All common column types already have a domain.\n");
    } else {
        b.push_str("\n#### Suggestions — common types without a domain\n\n");
        for (type_str, count) in &suggestions {
            b.push_str(&format!(
                "- **{type_str}** ({count} columns) — consider creating a domain, e.g. `CREATE DOMAIN D_{type_str} AS {type_str};`\n"
            ));
        }
    }

    Ok(b)
}

/// Finds columns by name pattern (supports % wildcards) across all tables.
pub fn handle_find_column(cfg: &Config, args: &Value) -> Result<String, String> {
    let pattern = arg_str(args, "column")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The 'column' argument is required (supports % wildcards).".to_string())?;

    let mut client = open_client(cfg, args)?;

    let sql = r#"
        SELECT
            TRIM(RF.RDB$RELATION_NAME) AS TABLE_NAME,
            TRIM(RF.RDB$FIELD_NAME) AS COLUMN_NAME,
            F.RDB$FIELD_TYPE,
            F.RDB$FIELD_SUB_TYPE,
            F.RDB$FIELD_LENGTH,
            F.RDB$FIELD_PRECISION,
            F.RDB$FIELD_SCALE,
            RF.RDB$NULL_FLAG,
            R.RDB$VIEW_BLR IS NOT NULL AS IS_VIEW
        FROM RDB$RELATION_FIELDS RF
        JOIN RDB$RELATIONS R ON RF.RDB$RELATION_NAME = R.RDB$RELATION_NAME
        JOIN RDB$FIELDS F ON RF.RDB$FIELD_SOURCE = F.RDB$FIELD_NAME
        WHERE RF.RDB$FIELD_NAME LIKE ?
          AND R.RDB$SYSTEM_FLAG = 0
        ORDER BY RF.RDB$RELATION_NAME, RF.RDB$FIELD_POSITION
    "#;

    let result = query::query_with_param(&mut client, sql, &pattern).map_err(|e| e.to_string())?;

    let mut b = String::new();
    b.push_str(&format!(
        "### Columns matching '{pattern}' ({} found)\n\n",
        result.rows.len()
    ));
    if result.rows.is_empty() {
        b.push_str("No columns found.\n");
        return Ok(b);
    }
    b.push_str("TABLE | COLUMN | TYPE | NULL | VIEW\n");
    b.push_str("--- | --- | --- | --- | ---\n");
    for row in &result.rows {
        let table = cell_text(row, 0);
        let column = cell_text(row, 1);
        let type_str = format_column_type(
            cell_int(row, 2),
            cell_int(row, 3),
            cell_int(row, 4),
            cell_int(row, 5),
            cell_int(row, 6),
        );
        let null_flag = cell_int(row, 7);
        let null_str = if is_not_null(Some(null_flag)) {
            "NO"
        } else {
            "YES"
        };
        let is_view = cell_int(row, 8);
        let view_str = if is_view == 1 { "YES" } else { "" };
        b.push_str(&format!(
            "{table} | {column} | {type_str} | {null_str} | {view_str}\n"
        ));
    }
    Ok(b)
}
