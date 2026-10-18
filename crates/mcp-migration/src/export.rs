//! Export Firebird tables, views, or SQL queries to Parquet files.
//!
//! All three sources (table, view, raw SQL) go through the same engine:
//! the source is normalised into a `SELECT` statement, executed via
//! [`query`], and the resulting [`ColumnMeta`] drives the Arrow schema.
//!
//! NUMERIC/DECIMAL columns arrive as text (vendor patch) and are mapped
//! to `Utf8` — the driver does not propagate precision/scale through
//! `ColumnMeta`.

use std::fs::File;
use std::io::BufWriter;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Decimal128Array, Float32Array, Float64Array,
    Int16Array, Int32Array, Int64Array, RecordBatch, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Schema, TimeUnit};
use chrono::{Datelike, NaiveDateTime, Timelike};
use parquet::arrow::ArrowWriter;

use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_core::query::{Value, query_unbounded};
use firebird_5_mcp_types::QueryResult;

use crate::types::column_meta_to_arrow;

/// Result of a Parquet export.
#[derive(Debug)]
pub struct ExportResult {
    pub path: String,
    pub rows: usize,
    pub columns: usize,
}

/// Exports a table, view, or raw SQL query to a data file.
///
/// If `source` starts with `SELECT` or `WITH` (case-insensitive), it is
/// treated as a raw SQL query. Otherwise it is treated as a table or view
/// name and wrapped in `SELECT * FROM "source"`.
///
/// `format` selects the output format: `"parquet"` or `"csv"`.
pub fn export(
    client: &mut FirebirdClient,
    source: &str,
    output_path: &str,
    format: &str,
) -> Result<ExportResult, String> {
    let sql = normalise_source(source);
    let result = query_unbounded(client, &sql).map_err(|e| e.to_string())?;
    if result.columns.is_empty() {
        return Err("query returned no columns".to_string());
    }

    let fmt = format.to_lowercase();
    match fmt.as_str() {
        "parquet" => {
            let schema = column_meta_to_arrow(&result.columns);
            let batch = build_record_batch(&schema, &result)?;
            write_parquet(output_path, &schema, &batch)?;
        }
        "csv" => {
            write_csv(output_path, &result)?;
        }
        other => {
            return Err(format!(
                "unsupported format '{other}' — only 'parquet' and 'csv' are supported"
            ));
        }
    }

    mlog!(
        "{fmt} export: exported {} rows to {}",
        result.rows.len(),
        output_path
    );

    Ok(ExportResult {
        path: output_path.to_string(),
        rows: result.rows.len(),
        columns: result.columns.len(),
    })
}

/// Writes a `QueryResult` to a CSV file (header + rows) using the `csv` crate.
fn write_csv(path: &str, result: &QueryResult) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
    let mut writer = csv::Writer::from_writer(file);

    // Header
    let header: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    writer
        .write_record(&header)
        .map_err(|e| format!("cannot write CSV header: {e}"))?;

    // Rows
    for row in &result.rows {
        let fields: Vec<String> = row.iter().map(|v| value_to_text(v)).collect();
        writer
            .write_record(&fields)
            .map_err(|e| format!("cannot write CSV row: {e}"))?;
    }

    writer
        .flush()
        .map_err(|e| format!("cannot flush CSV: {e}"))?;
    Ok(())
}

/// Normalises a source string into a SELECT statement.
///
/// - `SELECT ...` / `WITH ...` → used as-is (raw SQL)
/// - anything else → treated as a table/view name → `SELECT * FROM "name"`
fn normalise_source(source: &str) -> String {
    let trimmed = source.trim();
    let upper = trimmed.to_uppercase();
    if upper.starts_with("SELECT") || upper.starts_with("WITH") {
        trimmed.to_string()
    } else {
        format!("SELECT * FROM \"{trimmed}\"")
    }
}

/// Builds a `RecordBatch` from a `QueryResult` using the given Arrow schema.
///
/// Columns are matched positionally: the Nth field in the Arrow schema
/// corresponds to the Nth value in each row.
fn build_record_batch(schema: &Schema, result: &QueryResult) -> Result<RecordBatch, String> {
    let n = result.rows.len();
    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(schema.fields().len());

    for (col_idx, field) in schema.fields().iter().enumerate() {
        let col_values: Vec<&Value> = result
            .rows
            .iter()
            .map(|row| row.get(col_idx).unwrap_or(&Value::Null))
            .collect();

        let arr = build_column_array(field.data_type(), &col_values, n)?;
        arrays.push(arr);
    }

    RecordBatch::try_new(schema.clone().into(), arrays).map_err(|e| e.to_string())
}

/// Builds a single Arrow array column from Firebird values.
fn build_column_array(dt: &DataType, values: &[&Value], _n: usize) -> Result<ArrayRef, String> {
    match dt {
        DataType::Int16 => {
            let arr: Int16Array = values.iter().map(|v| Some(int_or_zero(v) as i16)).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Int32 => {
            let arr: Int32Array = values.iter().map(|v| Some(int_or_zero(v) as i32)).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Int64 => {
            let arr: Int64Array = values.iter().map(|v| Some(int_or_zero(v))).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Float32 => {
            let arr: Float32Array = values
                .iter()
                .map(|v| Some(float_or_zero(v) as f32))
                .collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Float64 => {
            let arr: Float64Array = values.iter().map(|v| Some(float_or_zero(v))).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Utf8 => {
            let arr: StringArray = values.iter().map(|v| Some(value_to_text(v))).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Boolean => {
            let arr: BooleanArray = values.iter().map(|v| Some(bool_or_false(v))).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Binary => {
            let owned: Vec<Vec<u8>> = values
                .iter()
                .map(|v| match v {
                    Value::Binary(data) => data.clone(),
                    Value::Null => Vec::new(),
                    other => value_to_text(other).into_bytes(),
                })
                .collect();
            let arr: BinaryArray = owned.iter().map(|b| Some(b.as_slice())).collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Date32 => {
            let arr: Date32Array = values
                .iter()
                .map(|v| match v {
                    Value::Date(d) => Some(d.num_days_from_ce()),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Time64(TimeUnit::Microsecond) => {
            let arr: Time64MicrosecondArray = values
                .iter()
                .map(|v| match v {
                    Value::Time(t) => Some(
                        t.num_seconds_from_midnight() as i64 * 1_000_000
                            + t.nanosecond() as i64 / 1000,
                    ),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Timestamp(TimeUnit::Microsecond, None) => {
            let arr: TimestampMicrosecondArray = values
                .iter()
                .map(|v| match v {
                    Value::Timestamp(ts) => Some(naive_dt_to_micros(ts)),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        DataType::Decimal128(_p, s) => {
            let arr: Decimal128Array = values
                .iter()
                .map(|v| match v {
                    Value::Null => None,
                    Value::Text(txt) => parse_decimal(txt, *s),
                    Value::Int(n) => Some(*n as i128),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(arr) as ArrayRef)
        }
        other => Err(format!("unsupported Arrow type: {other:?}")),
    }
}

/// Converts a `NaiveDateTime` to microseconds since Unix epoch.
fn naive_dt_to_micros(ts: &NaiveDateTime) -> i64 {
    let date = ts.date();
    let time = ts.time();
    let days = date.num_days_from_ce();
    let secs = time.num_seconds_from_midnight() as i64;
    let nanos = time.nanosecond() as i64;
    // Unix epoch is 1970-01-01, which is 719163 days from CE.
    let days_from_epoch = (days - 719_163) as i64;
    days_from_epoch * 86_400_000_000 + secs * 1_000_000 + nanos / 1000
}

/// Parses a decimal string (e.g. "12.3456") into an unscaled i128.
fn parse_decimal(s: &str, scale: i8) -> Option<i128> {
    let negative = s.starts_with('-');
    let s = s.strip_prefix('-').unwrap_or(s);
    let parts: Vec<&str> = s.split('.').collect();
    let int_part: i128 = parts[0].parse().ok()?;
    let frac_part: i128 = parts
        .get(1)
        .map(|p| p.parse().ok().unwrap_or(0))
        .unwrap_or(0);
    let unscaled = if scale < 0 {
        int_part * 10i128.pow((-scale) as u32) + frac_part
    } else {
        int_part
    };
    Some(if negative { -unscaled } else { unscaled })
}

/// Writes a `RecordBatch` to a Parquet file.
fn write_parquet(path: &str, schema: &Schema, batch: &RecordBatch) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
    let writer = BufWriter::new(file);
    let mut writer = ArrowWriter::try_new(writer, Arc::new(schema.clone()), None)
        .map_err(|e| format!("cannot open Parquet writer: {e}"))?;
    writer
        .write(batch)
        .map_err(|e| format!("cannot write Parquet batch: {e}"))?;
    writer
        .close()
        .map_err(|e| format!("cannot close Parquet file: {e}"))?;
    Ok(())
}

// --- Value helpers ---

fn value_to_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Text(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        Value::Time(t) => t.format("%H:%M:%S").to_string(),
        Value::Timestamp(ts) => ts.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::Binary(_) => "<BLOB>".to_string(),
    }
}

fn int_or_zero(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        Value::Text(s) => s.parse().unwrap_or(0),
        Value::Float(f) => *f as i64,
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

fn float_or_zero(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn bool_or_false(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Text(s) => s.eq_ignore_ascii_case("true") || s == "1",
        _ => false,
    }
}
