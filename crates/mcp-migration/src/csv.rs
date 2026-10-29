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
//!
//! Compression is transparent: the file extension determines the codec
//! (`.csv`, `.csv.zst`, `.csv.gz`, `.csv.lz4`).

use firebird_5_mcp_core::FirebirdClient;
use firebird_5_mcp_core::mlog;

use crate::format::{CodePage, Format};
use crate::import::{ImportMode, ImportResult, insert_rows_in_batches, keep_column_indices};
use crate::io::TextReader;
use crate::table::{create_table_from_types, table_exists};

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
    codepage: CodePage,
) -> Result<ImportResult, String> {
    codepage.validate_for(Format::Csv)?;

    // Read raw bytes, decompress, then decode with the specified codepage
    let raw_bytes = TextReader::open(input_path)?.into_bytes()?;
    let csv_text = codepage.decode(&raw_bytes);
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());

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
    let keep_indices = keep_column_indices(
        all_columns.len(),
        |i| all_columns[i].as_str(),
        ignore_columns,
    );

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
            create_table_from_types(client, table, &kept_columns, &inferred_types, &sample_rows)?;
        }
        ImportMode::Append => {
            if !table_exists(client, table)? {
                return Err(format!(
                    "table '{table}' not found (use mode 'create' to create it)"
                ));
            }
        }
    }

    // Insert all rows in batches
    let columns: Vec<String> = kept_columns.clone();
    let all_rows_iter = sample_rows.iter().chain(remaining_rows.iter()).map(|row| {
        let values: Vec<serde_json::Value> = row.iter().map(|v| csv_value_to_json(v)).collect();
        Ok(values)
    });
    let inserted = insert_rows_in_batches(client, table, &columns, all_rows_iter)?;

    mlog!("CSV import: imported {inserted} rows from {input_path} into {table}");

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: kept_columns.len(),
        mode,
    })
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
