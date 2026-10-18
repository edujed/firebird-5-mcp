//! CSV import for Firebird.
//!
//! CSV is untyped, so column types are inferred from a sample of the first
//! rows. Inference rules (checked in order):
//! - all empty → VARCHAR(255)
//! - all integers → INTEGER
//! - all floats → DOUBLE PRECISION
//! - all booleans → BOOLEAN
//! - all `YYYY-MM-DD` → DATE
//! - all `YYYY-MM-DD HH:MM:SS` → TIMESTAMP
//! - anything else → VARCHAR(255)

use std::fs::File;

use csv::ReaderBuilder;
use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_core::query::{Value, exec, insert_batch, query_with_param};

use crate::import::{ImportMode, ImportResult};

/// Number of sample rows to read for type inference.
const SAMPLE_SIZE: usize = 100;

/// Imports a CSV file into a Firebird table.
///
/// The first line is treated as the header (column names).
/// `ignore_columns` are skipped (case-insensitive).
pub fn import_csv(
    client: &mut FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    let file = File::open(input_path).map_err(|e| format!("cannot open {input_path}: {e}"))?;
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    // Read header
    let headers = reader
        .headers()
        .map_err(|e| format!("cannot read CSV header: {e}"))?
        .clone();
    let all_columns: Vec<String> = headers.iter().map(|h| h.trim().to_string()).collect();

    if all_columns.is_empty() {
        return Err("CSV header is empty".to_string());
    }

    // Determine which columns to keep
    let keep_indices: Vec<usize> = all_columns
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            !ignore_columns
                .iter()
                .any(|ic| ic.to_uppercase() == c.to_uppercase())
        })
        .map(|(i, _)| i)
        .collect();

    if keep_indices.is_empty() {
        return Err("all columns are in ignore_columns — nothing to import".to_string());
    }

    let kept_columns: Vec<String> = keep_indices
        .iter()
        .map(|&i| all_columns[i].clone())
        .collect();

    // Read all rows from the CSV
    let mut all_rows: Vec<Vec<String>> = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| format!("cannot read CSV row: {e}"))?;
        let row: Vec<String> = keep_indices
            .iter()
            .map(|&i| record.get(i).unwrap_or("").to_string())
            .collect();
        all_rows.push(row);
    }

    if all_rows.is_empty() {
        return Err("CSV file contains no data rows".to_string());
    }

    // Split into sample and remaining
    let sample_rows: Vec<Vec<String>> = all_rows.iter().take(SAMPLE_SIZE).cloned().collect();
    let remaining_rows: Vec<Vec<String>> = all_rows.iter().skip(SAMPLE_SIZE).cloned().collect();

    // Infer types
    let inferred_types: Vec<String> = kept_columns
        .iter()
        .enumerate()
        .map(|(col_idx, _)| {
            let values: Vec<&str> = sample_rows.iter().map(|r| r[col_idx].as_str()).collect();
            infer_type(&values)
        })
        .collect();

    // Create or verify table
    match mode {
        ImportMode::Create => {
            create_table_csv(client, table, &kept_columns, &inferred_types, &sample_rows)?;
        }
        ImportMode::Append => {
            verify_table_exists(client, table)?;
        }
    }

    // Insert sample rows
    let mut inserted = 0;
    let columns: Vec<String> = kept_columns.clone();
    for row in &sample_rows {
        let values: Vec<serde_json::Value> = row.iter().map(|v| csv_value_to_json(v)).collect();
        let n = insert_batch(client, table, &columns, &[values]).map_err(|e| e.to_string())?;
        inserted += n;
    }

    // Insert remaining rows (batches of 500)
    let mut batch: Vec<Vec<serde_json::Value>> = Vec::with_capacity(500);
    for row in &remaining_rows {
        let values: Vec<serde_json::Value> = row.iter().map(|v| csv_value_to_json(v)).collect();
        batch.push(values);
        if batch.len() >= 500 {
            let n = insert_batch(client, table, &columns, &batch).map_err(|e| e.to_string())?;
            inserted += n;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        let n = insert_batch(client, table, &columns, &batch).map_err(|e| e.to_string())?;
        inserted += n;
    }

    mlog!("CSV import: imported {inserted} rows from {input_path} into {table}");

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: kept_columns.len(),
        mode,
    })
}

/// Creates a table from inferred CSV column types.
fn create_table_csv(
    client: &mut FirebirdClient,
    table: &str,
    columns: &[String],
    types: &[String],
    sample_rows: &[Vec<String>],
) -> Result<(), String> {
    let check_sql = "SELECT COUNT(*) FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = ?";
    let result = query_with_param(client, check_sql, table).map_err(|e| e.to_string())?;
    let count = result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| match v {
            Value::Int(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    if count > 0 {
        return Err(format!(
            "table '{table}' already exists (use mode 'append' to add data)"
        ));
    }

    let col_defs: Vec<String> = columns
        .iter()
        .enumerate()
        .zip(types.iter())
        .map(|((col_idx, name), ty)| {
            // Check if this column has any empty values in the sample
            let has_empty = sample_rows.iter().any(|row| row[col_idx].is_empty());
            // Firebird columns are nullable by default; only add NOT NULL if no empty values
            if has_empty {
                format!("\"{name}\" {ty}")
            } else {
                format!("\"{name}\" {ty} NOT NULL")
            }
        })
        .collect();
    let ddl = format!("CREATE TABLE \"{table}\" ({})", col_defs.join(", "));
    exec(client, &ddl).map_err(|e| format!("CREATE TABLE failed: {e}"))?;
    Ok(())
}

/// Verifies the table exists (for append mode).
fn verify_table_exists(client: &mut FirebirdClient, table: &str) -> Result<(), String> {
    let check_sql = "SELECT COUNT(*) FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = ?";
    let result = query_with_param(client, check_sql, table).map_err(|e| e.to_string())?;
    let count = result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| match v {
            Value::Int(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    if count == 0 {
        return Err(format!(
            "table '{table}' not found (use mode 'create' to create it)"
        ));
    }
    Ok(())
}

/// Infers a Firebird DDL type from sample values.
fn infer_type(values: &[&str]) -> String {
    let non_empty: Vec<&str> = values.iter().copied().filter(|v| !v.is_empty()).collect();
    if non_empty.is_empty() {
        return "VARCHAR(255)".to_string();
    }

    if non_empty.iter().all(|v| is_integer(v)) {
        return "INTEGER".to_string();
    }
    if non_empty.iter().all(|v| is_float(v)) {
        return "DOUBLE PRECISION".to_string();
    }
    if non_empty.iter().all(|v| is_boolean(v)) {
        return "BOOLEAN".to_string();
    }
    if non_empty.iter().all(|v| is_date(v)) {
        return "DATE".to_string();
    }
    if non_empty.iter().all(|v| is_timestamp(v)) {
        return "TIMESTAMP".to_string();
    }

    "VARCHAR(255)".to_string()
}

fn is_integer(v: &str) -> bool {
    let v = v.trim();
    if v.is_empty() {
        return true;
    }
    let start = if v.starts_with('-') { 1 } else { 0 };
    v.len() > start && v[start..].chars().all(|c| c.is_ascii_digit())
}

fn is_float(v: &str) -> bool {
    let v = v.trim();
    if v.is_empty() {
        return true;
    }
    v.parse::<f64>().is_ok()
}

fn is_boolean(v: &str) -> bool {
    let v = v.trim().to_lowercase();
    v == "true" || v == "false" || v == "1" || v == "0"
}

fn is_date(v: &str) -> bool {
    let v = v.trim();
    if v.len() != 10 {
        return false;
    }
    v.get(4..5) == Some("-")
        && v.get(7..8) == Some("-")
        && v[..4].chars().all(|c| c.is_ascii_digit())
        && v[5..7].chars().all(|c| c.is_ascii_digit())
        && v[8..].chars().all(|c| c.is_ascii_digit())
}

fn is_timestamp(v: &str) -> bool {
    let v = v.trim();
    if v.len() != 19 {
        return false;
    }
    v.get(4..5) == Some("-")
        && v.get(7..8) == Some("-")
        && v.get(10..11) == Some(" ")
        && v.get(13..14) == Some(":")
        && v.get(16..17) == Some(":")
}

/// Converts a CSV string value to a JSON value for `insert_batch`.
fn csv_value_to_json(v: &str) -> serde_json::Value {
    let v = v.trim();
    if v.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::Value::String(v.to_string())
}
