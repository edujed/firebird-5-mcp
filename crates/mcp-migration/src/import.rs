//! Import Parquet files into Firebird tables.
//!
//! Two modes:
//! - `create` — creates the table from the Parquet schema, then inserts data.
//!   Fails if the table already exists.
//! - `append` — the table must already exist with a compatible structure.
//!   Data is inserted into the existing table.

use std::fs::File;

use arrow::array::{
    Array, BinaryArray, BooleanArray, Date32Array, Decimal128Array, Float32Array, Float64Array,
    Int16Array, Int32Array, Int64Array, RecordBatch, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Schema};
use chrono::NaiveDate;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_core::query::{Value, exec, insert_batch, query_with_param};

use crate::types::{arrow_to_firebird, check_append_compatibility};

/// Import mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Create the table (fails if it already exists), then insert.
    Create,
    /// Table must already exist; insert data into it.
    Append,
}

impl ImportMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "create" => Ok(Self::Create),
            "append" => Ok(Self::Append),
            other => Err(format!(
                "invalid import mode '{other}' — expected 'create' or 'append'"
            )),
        }
    }
}

/// Result of a Parquet import.
#[derive(Debug)]
pub struct ImportResult {
    pub table: String,
    pub rows: usize,
    pub columns: usize,
    pub mode: ImportMode,
}

/// Imports a Parquet file into a Firebird table.
///
/// `ignore_columns` is an optional list of column names (case-insensitive) to
/// skip. In `create` mode they are excluded from the DDL; in `append` mode
/// they are excluded from the INSERT column list and the compatibility check.
pub fn import_parquet(
    client: &mut FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    // Read the Parquet file
    let file = File::open(input_path).map_err(|e| format!("cannot open {input_path}: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("cannot read Parquet file: {e}"))?;
    let full_schema: Schema = (**builder.schema()).clone();
    let batches: Vec<RecordBatch> = builder
        .build()
        .map_err(|e| format!("cannot build Parquet reader: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot read Parquet batches: {e}"))?;

    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    if total_rows == 0 {
        return Err("Parquet file contains no rows".to_string());
    }

    // Determine which column indices to keep (exclude ignored columns)
    let keep_indices: Vec<usize> = full_schema
        .fields()
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            !ignore_columns
                .iter()
                .any(|ic| ic.to_uppercase() == f.name().to_uppercase())
        })
        .map(|(i, _)| i)
        .collect();

    if keep_indices.is_empty() {
        return Err("all columns are in ignore_columns — nothing to import".to_string());
    }

    // Build a filtered schema containing only the kept columns
    let filtered_fields: Vec<arrow::datatypes::Field> = keep_indices
        .iter()
        .map(|&i| full_schema.field(i).clone())
        .collect();
    let schema = arrow::datatypes::Schema::new(filtered_fields);

    match mode {
        ImportMode::Create => {
            create_table(client, table, &schema)?;
        }
        ImportMode::Append => {
            verify_append(client, table, &schema)?;
        }
    }

    // Insert data in batches (only kept columns)
    let mut inserted = 0;
    for batch in &batches {
        let rows = batch_to_rows_filtered(batch, &full_schema, &keep_indices)?;
        let columns: Vec<String> = keep_indices
            .iter()
            .map(|&i| full_schema.fields()[i].name().clone())
            .collect();
        let n = insert_batch(client, table, &columns, &rows).map_err(|e| e.to_string())?;
        inserted += n;
    }

    mlog!(
        "Parquet import: imported {} rows from {} into {}",
        inserted,
        input_path,
        table
    );

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: keep_indices.len(),
        mode,
    })
}

/// Creates a table from the Parquet schema. Fails if the table already exists.
fn create_table(client: &mut FirebirdClient, table: &str, schema: &Schema) -> Result<(), String> {
    // Check if table exists
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

    // Build DDL
    let col_defs: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| {
            let fb_type = arrow_to_firebird(f.data_type());
            format!("\"{}\" {}", f.name(), fb_type)
        })
        .collect();
    let ddl = format!("CREATE TABLE \"{}\" ({})", table, col_defs.join(", "));
    exec(client, &ddl).map_err(|e| format!("CREATE TABLE failed: {e}"))?;

    Ok(())
}

/// Verifies that the table exists and its structure is compatible with the
/// Parquet schema for `append` mode.
fn verify_append(client: &mut FirebirdClient, table: &str, schema: &Schema) -> Result<(), String> {
    // Get table columns from RDB$
    let sql = r#"
        SELECT RF.RDB$FIELD_NAME, F.RDB$FIELD_TYPE
        FROM RDB$RELATION_FIELDS RF
        JOIN RDB$FIELDS F ON F.RDB$FIELD_NAME = RF.RDB$FIELD_SOURCE
        WHERE RF.RDB$RELATION_NAME = ?
        ORDER BY RF.RDB$FIELD_POSITION
    "#;
    let result = query_with_param(client, sql, table).map_err(|e| e.to_string())?;

    if result.rows.is_empty() {
        return Err(format!(
            "table '{table}' not found (use mode 'create' to create it)"
        ));
    }

    let fb_columns: Vec<(String, i64)> = result
        .rows
        .iter()
        .map(|row| {
            let name = row
                .first()
                .and_then(|v| match v {
                    Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let type_num = row
                .get(1)
                .and_then(|v| match v {
                    Value::Int(n) => Some(*n),
                    _ => None,
                })
                .unwrap_or(0);
            (name, type_num)
        })
        .collect();

    check_append_compatibility(schema, &fb_columns)?;
    Ok(())
}

/// Converts an Arrow `RecordBatch` to rows of `serde_json::Value` for
/// `insert_batch`, including only the columns at `keep_indices`.
fn batch_to_rows_filtered(
    batch: &RecordBatch,
    schema: &Schema,
    keep_indices: &[usize],
) -> Result<Vec<Vec<serde_json::Value>>, String> {
    let mut rows = Vec::with_capacity(batch.num_rows());

    for row_idx in 0..batch.num_rows() {
        let mut row = Vec::with_capacity(keep_indices.len());
        for &col_idx in keep_indices {
            let field = &schema.fields()[col_idx];
            let array = &batch.columns()[col_idx];
            let v = arrow_value_to_json(array, field.data_type(), row_idx)?;
            row.push(v);
        }
        rows.push(row);
    }

    Ok(rows)
}

/// Converts a single Arrow array value to a `serde_json::Value`.
fn arrow_value_to_json(
    array: &arrow::array::ArrayRef,
    dt: &DataType,
    idx: usize,
) -> Result<serde_json::Value, String> {
    if array.is_null(idx) {
        return Ok(serde_json::Value::Null);
    }

    Ok(match dt {
        DataType::Int16 => {
            let a = downcast::<Int16Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Int32 => {
            let a = downcast::<Int32Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Int64 => {
            let a = downcast::<Int64Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Float32 => {
            let a = downcast::<Float32Array>(array)?;
            serde_json::Value::Number(
                serde_json::Number::from_f64(a.value(idx) as f64).ok_or("invalid float")?,
            )
        }
        DataType::Float64 => {
            let a = downcast::<Float64Array>(array)?;
            serde_json::Value::Number(
                serde_json::Number::from_f64(a.value(idx)).ok_or("invalid float")?,
            )
        }
        DataType::Utf8 => {
            let a = downcast::<StringArray>(array)?;
            serde_json::Value::String(a.value(idx).to_string())
        }
        DataType::Boolean => {
            let a = downcast::<BooleanArray>(array)?;
            serde_json::Value::Bool(a.value(idx))
        }
        DataType::Binary => {
            let a = downcast::<BinaryArray>(array)?;
            // Binary data: encode as hex string (Firebird will interpret as BLOB)
            serde_json::Value::String(a.value(idx).iter().map(|b| format!("{b:02x}")).collect())
        }
        DataType::Date32 => {
            let a = downcast::<Date32Array>(array)?;
            let days = a.value(idx);
            let date = NaiveDate::from_num_days_from_ce_opt(days)
                .ok_or_else(|| format!("invalid date: {days} days from CE"))?;
            serde_json::Value::String(date.format("%Y-%m-%d").to_string())
        }
        DataType::Time64(_) => {
            let a = downcast::<Time64MicrosecondArray>(array)?;
            let micros = a.value(idx);
            let secs = micros / 1_000_000;
            let frac = micros % 1_000_000;
            serde_json::Value::String(format!(
                "{:02}:{:02}:{:02}.{:06}",
                secs / 3600,
                (secs / 60) % 60,
                secs % 60,
                frac
            ))
        }
        DataType::Timestamp(_, _) => {
            let a = downcast::<TimestampMicrosecondArray>(array)?;
            let micros = a.value(idx);
            let dt = chrono::DateTime::from_timestamp_micros(micros)
                .ok_or_else(|| format!("invalid timestamp: {micros}"))?;
            serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        DataType::Decimal128(_, s) => {
            let a = downcast::<Decimal128Array>(array)?;
            let unscaled = a.value(idx);
            let scale = *s as i32;
            let s = format_decimal(unscaled, scale);
            serde_json::Value::String(s)
        }
        other => {
            return Err(format!("unsupported Arrow type: {other:?}"));
        }
    })
}

/// Formats a Decimal128 unscaled value with the given scale.
fn format_decimal(unscaled: i128, scale: i32) -> String {
    if scale == 0 {
        return unscaled.to_string();
    }
    let negative = unscaled < 0;
    let digits = unscaled.unsigned_abs().to_string();
    let body = if scale > 0 {
        let padded = format!("{:0>width$}", digits, width = scale as usize + 1);
        let split = padded.len() - scale as usize;
        format!("{}.{}", &padded[..split], &padded[split..])
    } else {
        let frac_len = (-scale) as usize;
        let padded = format!("{:0>width$}", digits, width = frac_len + 1);
        let split = padded.len() - frac_len;
        format!("{}.{}", &padded[..split], &padded[split..])
    };
    if negative { format!("-{body}") } else { body }
}

/// Downcasts an `ArrayRef` to a concrete Arrow array type.
fn downcast<T: Array + 'static>(array: &arrow::array::ArrayRef) -> Result<&T, String> {
    array
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| format!("cannot downcast to {:?}", std::any::TypeId::of::<T>()))
}
