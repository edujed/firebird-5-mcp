//! JSON Lines (JSONL / NDJSON / LDJSON) export, import, and describe.
//!
//! Each row is a single JSON object on its own line:
//! ```json
//! {"ID": 1, "NAME": "Alice", "EMAIL": "a@x.com"}
//! {"ID": 2, "NAME": "Bob", "EMAIL": null}
//! ```
//!
//! - NULL → `null`
//! - NUMERIC/DECIMAL → string (preserves exact decimal from vendor patch)
//! - TIMESTAMP → ISO 8601 string
//! - BLOB → base64 string
//!
//! Compression is transparent: the file extension determines the codec
//! (`.jsonl`, `.jsonl.zst`, `.jsonl.gz`, `.jsonl.lz4`).
//! Also recognises `.ndjson` and `.ldjson` extensions.
//!
//! Import type inference (from first 100 rows):
//! - all integers → INTEGER
//! - all numbers → DOUBLE PRECISION
//! - all booleans → BOOLEAN
//! - all `YYYY-MM-DD` strings → DATE
//! - all `YYYY-MM-DD HH:MM:SS` strings → TIMESTAMP
//! - anything else → VARCHAR(255)

use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_core::query::Value;
use firebird_5_mcp_types::QueryResult;

use crate::import::{ImportMode, ImportResult, insert_rows_in_batches, keep_column_indices};
use crate::io::{TextReader, TextWriter};
use crate::table::{create_table_from_types, table_exists};

/// Number of sample rows to read for type inference.
const SAMPLE_SIZE: usize = 100;

/// Number of sample rows to show in describe.
const DESCRIBE_SAMPLE_ROWS: usize = 5;

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Writes a `QueryResult` to a JSON Lines file.
///
/// Compression is determined by the file extension (see
/// [`crate::format::file_compression_from_path`]).
pub fn write_jsonl(path: &str, result: &QueryResult) -> Result<(), String> {
    let mut content = String::new();

    for row in &result.rows {
        let mut map = serde_json::Map::new();
        for (i, col) in result.columns.iter().enumerate() {
            let v = row.get(i).unwrap_or(&Value::Null);
            map.insert(col.name.clone(), value_to_json(v));
        }
        content
            .push_str(&serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_default());
        content.push('\n');
    }

    let mut sink = TextWriter::create(path)?;
    sink.write_bytes(content.as_bytes())?;
    let inner = sink.into_inner();
    TextWriter::finish(inner)?;
    Ok(())
}

/// Converts a Firebird `Value` to a `serde_json::Value` for JSON Lines export.
fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Int(n) => serde_json::Value::Number(serde_json::Number::from(*n)),
        // Text is always a JSON string. NUMERIC/DECIMAL arrives as exact
        // decimal text (vendor patch) and must stay a string to preserve
        // precision (e.g. "80.00"); a VARCHAR containing "123" must not
        // silently become the JSON number 123. On import, Firebird coerces
        // text parameters to the target column type.
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Date(d) => serde_json::Value::String(d.format("%Y-%m-%d").to_string()),
        Value::Time(t) => serde_json::Value::String(t.format("%H:%M:%S").to_string()),
        Value::Timestamp(ts) => {
            serde_json::Value::String(ts.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Binary(data) => serde_json::Value::String(base64_encode(data)),
    }
}

/// Minimal base64 encoder (avoids adding a dependency).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|b| *b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|b| *b as u32).unwrap_or(0);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(triple >> 18) as usize & 0x3F] as char);
        out.push(CHARS[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            CHARS[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/// Imports a JSON Lines file into a Firebird table.
///
/// Each line is a JSON object. Column names come from the keys of the first
/// row. `ignore_columns` are skipped (case-insensitive).
///
/// Type inference uses the first 100 rows (same strategy as CSV).
pub fn import_jsonl(
    client: &mut FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    // Read raw bytes, decompress
    let raw_bytes = TextReader::open(input_path)?.into_bytes()?;
    let text = String::from_utf8_lossy(&raw_bytes);

    // Parse all lines into JSON objects
    let mut all_rows: Vec<serde_json::Value> = Vec::new();
    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("line {}: invalid JSON: {e}", line_num + 1))?;
        if !parsed.is_object() {
            return Err(format!(
                "line {}: expected a JSON object, got {}",
                line_num + 1,
                parsed
            ));
        }
        all_rows.push(parsed);
    }

    if all_rows.is_empty() {
        return Err("JSON Lines file contains no data rows".to_string());
    }

    // Collect column names from the first row (JSON objects preserve key order
    // via serde_json::Map with the "preserve_order" feature — but we use a
    // BTreeMap by default, so we collect from the first row's keys).
    let first = &all_rows[0];
    let all_columns: Vec<String> = first
        .as_object()
        .ok_or("first row is not a JSON object")?
        .keys()
        .cloned()
        .collect();

    if all_columns.is_empty() {
        return Err("first row has no columns".to_string());
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

    // Extract sample values for type inference
    let sample_rows: Vec<Vec<serde_json::Value>> = all_rows
        .iter()
        .take(SAMPLE_SIZE)
        .map(|row| {
            keep_indices
                .iter()
                .map(|&i| {
                    row.get(&all_columns[i])
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect()
        })
        .collect();

    // Infer types
    let inferred_types: Vec<String> = kept_columns
        .iter()
        .enumerate()
        .map(|(col_idx, _)| {
            let values: Vec<&serde_json::Value> = sample_rows.iter().map(|r| &r[col_idx]).collect();
            infer_jsonl_type(&values)
        })
        .collect();

    // Create or verify table
    match mode {
        ImportMode::Create => {
            // Convert sample values to strings for create_table_from_types
            let sample_strings: Vec<Vec<String>> = sample_rows
                .iter()
                .map(|row| row.iter().map(|v| json_value_to_string(v)).collect())
                .collect();
            create_table_from_types(
                client,
                table,
                &kept_columns,
                &inferred_types,
                &sample_strings,
            )?;
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
    let all_rows_iter = all_rows.iter().map(|row| {
        let values: Vec<serde_json::Value> = keep_indices
            .iter()
            .map(|&i| {
                row.get(&all_columns[i])
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
            })
            .collect();
        Ok(values)
    });
    let inserted = insert_rows_in_batches(client, table, &columns, all_rows_iter)?;

    mlog!("JSONL import: imported {inserted} rows from {input_path} into {table}");

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: kept_columns.len(),
        mode,
    })
}

/// Infers a Firebird DDL type from sample JSON values.
fn infer_jsonl_type(values: &[&serde_json::Value]) -> String {
    let non_null: Vec<&serde_json::Value> =
        values.iter().copied().filter(|v| !v.is_null()).collect();

    if non_null.is_empty() {
        return "VARCHAR(255)".to_string();
    }

    // All integers?
    if non_null
        .iter()
        .all(|v| matches!(v, serde_json::Value::Number(n) if n.as_i64().is_some()))
    {
        return "INTEGER".to_string();
    }

    // All numbers (int or float)?
    if non_null
        .iter()
        .all(|v| matches!(v, serde_json::Value::Number(_)))
    {
        return "DOUBLE PRECISION".to_string();
    }

    // All booleans?
    if non_null
        .iter()
        .all(|v| matches!(v, serde_json::Value::Bool(_)))
    {
        return "BOOLEAN".to_string();
    }

    // All strings that look like dates?
    if non_null
        .iter()
        .all(|v| matches!(v, serde_json::Value::String(s) if is_date_str(s)))
    {
        return "DATE".to_string();
    }

    // All strings that look like timestamps?
    if non_null
        .iter()
        .all(|v| matches!(v, serde_json::Value::String(s) if is_timestamp_str(s)))
    {
        return "TIMESTAMP".to_string();
    }

    "VARCHAR(255)".to_string()
}

fn is_date_str(v: &str) -> bool {
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

fn is_timestamp_str(v: &str) -> bool {
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

/// Converts a JSON value to a display string (for `create_table_from_types`
/// NOT NULL detection).
fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Describe
// ---------------------------------------------------------------------------

/// Describes a JSON Lines file: column names, inferred types, row count,
/// and first 5 sample rows.
pub fn describe_jsonl(path: &str) -> Result<crate::describe::FileDescription, String> {
    let raw_bytes = TextReader::open(path)?.into_bytes()?;
    let text = String::from_utf8_lossy(&raw_bytes);

    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed);
        }
    }

    if lines.is_empty() {
        return Err("JSON Lines file contains no data rows".to_string());
    }

    // Parse first row to get column names
    let first: serde_json::Value =
        serde_json::from_str(lines[0]).map_err(|e| format!("line 1: invalid JSON: {e}"))?;
    let columns: Vec<String> = first
        .as_object()
        .ok_or("first row is not a JSON object")?
        .keys()
        .cloned()
        .collect();

    // Parse sample rows (first 5)
    let mut sample_rows: Vec<Vec<String>> = Vec::with_capacity(DESCRIBE_SAMPLE_ROWS);
    for line in lines.iter().take(DESCRIBE_SAMPLE_ROWS) {
        let obj: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("invalid JSON: {e}"))?;
        let row: Vec<String> = columns
            .iter()
            .map(|col| {
                obj.get(col)
                    .map(|v| json_value_to_string(v))
                    .unwrap_or_default()
            })
            .collect();
        sample_rows.push(row);
    }

    // Infer types from first 100 rows
    let sample_values: Vec<Vec<serde_json::Value>> = lines
        .iter()
        .take(SAMPLE_SIZE)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|obj: serde_json::Value| {
            columns
                .iter()
                .map(|col| obj.get(col).cloned().unwrap_or(serde_json::Value::Null))
                .collect()
        })
        .collect();

    let arrow_types: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let values: Vec<&serde_json::Value> = sample_values.iter().map(|r| &r[i]).collect();
            infer_jsonl_type(&values)
        })
        .collect();

    let firebird_types: Vec<String> = vec![String::new(); columns.len()];

    Ok(crate::describe::FileDescription {
        path: path.to_string(),
        format: "jsonl".to_string(),
        rows: lines.len() as i64,
        columns,
        arrow_types,
        firebird_types,
        sample_rows,
    })
}
