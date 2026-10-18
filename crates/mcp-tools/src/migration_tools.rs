//! Migration tools: export tables/views/SQL to data files, import files to tables,
//! and describe file schemas.

use crate::ToolResult;
use crate::common::{arg_str, open_client};
use firebird_5_mcp_core::Config;
use firebird_5_mcp_migration::{ImportMode, describe_file, export, import};

const SUPPORTED_FORMATS: &str = "'parquet' and 'csv'";

/// `firebird_export` — export a table, view, or SQL query to a data file.
pub fn handle_export(config: &Config, args: &serde_json::Value) -> Result<ToolResult, String> {
    let source =
        arg_str(args, "source").ok_or_else(|| "missing required argument: source".to_string())?;
    let output_path = arg_str(args, "output_path")
        .ok_or_else(|| "missing required argument: output_path".to_string())?;
    let format = arg_str(args, "format").unwrap_or("parquet".to_string());
    let fmt = format.to_lowercase();

    if fmt != "parquet" && fmt != "csv" {
        return Err(format!(
            "unsupported format '{format}' — supported: {SUPPORTED_FORMATS}"
        ));
    }

    let mut client = open_client(config, args)?;
    let result = export(&mut client, &source, &output_path, &fmt)?;

    let text = format!(
        "Exported {} rows × {} columns to `{}` (format: {fmt})",
        result.rows, result.columns, result.path
    );

    let structured = serde_json::json!({
        "path": result.path,
        "rows": result.rows,
        "columns": result.columns,
        "format": fmt,
    });

    Ok(ToolResult::with_structured(text, structured))
}

/// `firebird_import` — import a data file into a Firebird table.
pub fn handle_import(config: &Config, args: &serde_json::Value) -> Result<ToolResult, String> {
    let input_path = arg_str(args, "input_path")
        .ok_or_else(|| "missing required argument: input_path".to_string())?;
    let table =
        arg_str(args, "table").ok_or_else(|| "missing required argument: table".to_string())?;
    let mode_str =
        arg_str(args, "mode").ok_or_else(|| "missing required argument: mode".to_string())?;
    let mode = ImportMode::parse(&mode_str)?;
    let format = arg_str(args, "format").unwrap_or("parquet".to_string());
    let fmt = format.to_lowercase();

    if fmt != "parquet" && fmt != "csv" {
        return Err(format!(
            "unsupported format '{format}' — supported: {SUPPORTED_FORMATS}"
        ));
    }

    let ignore_columns: Vec<String> = args
        .get("ignore_columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut client = open_client(config, args)?;

    let result = import(
        &mut client,
        &input_path,
        &table.to_uppercase(),
        mode,
        &fmt,
        &ignore_columns,
    )?;

    let mode_label = match result.mode {
        ImportMode::Create => "created table + inserted",
        ImportMode::Append => "appended to existing table",
    };

    let text = format!(
        "Imported {} rows × {} columns into `{}` ({}, format: {fmt})",
        result.rows, result.columns, result.table, mode_label
    );

    let structured = serde_json::json!({
        "table": result.table,
        "rows": result.rows,
        "columns": result.columns,
        "mode": mode_str,
        "format": fmt,
        "ignored_columns": ignore_columns,
    });

    Ok(ToolResult::with_structured(text, structured))
}

/// `firebird_describe_file` — describe a data file's schema without loading data.
pub fn handle_describe_file(
    _config: &Config,
    args: &serde_json::Value,
) -> Result<ToolResult, String> {
    let path =
        arg_str(args, "path").ok_or_else(|| "missing required argument: path".to_string())?;
    let format = arg_str(args, "format").unwrap_or("parquet".to_string());
    let fmt = format.to_lowercase();

    if fmt != "parquet" && fmt != "csv" {
        return Err(format!(
            "unsupported format '{format}' — supported: {SUPPORTED_FORMATS}"
        ));
    }

    let desc = describe_file(&path, &fmt)?;

    // Render as Markdown
    let mut text = String::new();
    text.push_str(&format!(
        "## File: `{}`\n\nFormat: **{}** | Rows: **{}** | Columns: **{}**\n\n",
        desc.path,
        desc.format,
        desc.rows,
        desc.columns.len()
    ));

    if desc.format == "csv" {
        // CSV: show column names + sample rows
        text.push_str("### Columns\n\n");
        for (i, col) in desc.columns.iter().enumerate() {
            text.push_str(&format!("{}. `{}`\n", i + 1, col));
        }

        if !desc.sample_rows.is_empty() {
            text.push_str("\n### Sample (first 5 rows)\n\n");
            // Header row
            text.push_str("| ");
            for col in &desc.columns {
                text.push_str(&format!("{col} | "));
            }
            text.push('\n');
            text.push_str("|");
            for _ in &desc.columns {
                text.push_str("---------|");
            }
            text.push('\n');
            // Data rows
            for row in &desc.sample_rows {
                text.push_str("| ");
                for (i, _) in desc.columns.iter().enumerate() {
                    let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    text.push_str(&format!("{val} | "));
                }
                text.push('\n');
            }
        }
    } else {
        // Parquet: show types table
        text.push_str("| # | Column | Arrow Type | Firebird Type |\n");
        text.push_str("|---|--------|-----------|---------------|\n");
        for i in 0..desc.columns.len() {
            text.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                i + 1,
                desc.columns[i],
                desc.arrow_types[i],
                desc.firebird_types[i]
            ));
        }
    }

    let structured = serde_json::json!({
        "path": desc.path,
        "format": desc.format,
        "rows": desc.rows,
        "columns": desc.columns,
        "arrow_types": desc.arrow_types,
        "firebird_types": desc.firebird_types,
        "sample_rows": desc.sample_rows,
    });

    Ok(ToolResult::with_structured(text, structured))
}
