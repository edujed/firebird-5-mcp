//! Export engine: table/view/SQL → data file.
//!
//! All three sources (table, view, raw SQL) go through the same path:
//! the source is normalised into a `SELECT` statement, executed via
//! [`query_unbounded`] (no row cap — export must fetch ALL rows), and the
//! resulting [`ColumnMeta`] drives the Arrow schema.
//!
//! NUMERIC/DECIMAL columns arrive as exact decimal text (vendor patch) and
//! are mapped to `Decimal128(p, s)` using the precision/scale inferred by the
//! query layer — `ColumnMeta.is_numeric` distinguishes them from real BIGINT
//! columns.

use std::sync::Arc;

use arrow::array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Decimal128Array, Float32Array, Float64Array,
    Int16Array, Int32Array, Int64Array, RecordBatch, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Schema, TimeUnit};
use arrow::ipc::CompressionType;
use arrow::ipc::writer::{FileWriter as IpcFileWriter, IpcWriteOptions};
use chrono::{Datelike, NaiveDateTime, Timelike};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use firebird_5_mcp_core::{FirebirdClient, mlog};
use mcp_core::query::query_unbounded;
use mcp_types::{QueryResult, Value};

use crate::dbf::write_dbase;
use crate::format::{CodePage, Format};
use crate::io::TextWriter;
use crate::jsonl::write_jsonl;
use crate::types::column_meta_to_arrow;
use crate::value::{bool_or_false, float_or_zero, int_or_zero, value_to_text};

/// Result of a file export.
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
/// `format` selects the output format: `"parquet"`, `"ipc"`, or `"csv"`.
///
/// `compression` is an optional override: `"snappy"`, `"zstd"`, `"lz4"`, or
/// `"none"`. When `None`, the format default is used (Parquet → Snappy,
/// IPC → ZSTD, CSV → determined by file extension).
pub fn export(
    client: &mut FirebirdClient,
    source: &str,
    output_path: &str,
    format: &str,
    compression: Option<&str>,
    codepage: Option<&str>,
) -> Result<ExportResult, String> {
    let fmt = Format::parse(format)?;
    let cp = match codepage {
        Some(s) => CodePage::parse(s)?,
        None => CodePage::default_for(fmt),
    };
    let sql = normalise_source(source);
    let result = query_unbounded(client, &sql).map_err(|e| e.to_string())?;
    if result.columns.is_empty() {
        return Err("query returned no columns".to_string());
    }

    match fmt {
        Format::Parquet => {
            let schema = column_meta_to_arrow(&result.columns);
            let batch = build_record_batch(&schema, &result)?;
            write_parquet(output_path, &schema, &batch, compression)?;
        }
        Format::Ipc => {
            let schema = column_meta_to_arrow(&result.columns);
            let batch = build_record_batch(&schema, &result)?;
            write_ipc(output_path, &schema, &batch, compression)?;
        }
        Format::Csv => {
            write_csv(output_path, &result, cp)?;
        }
        Format::Dbase => {
            write_dbase(output_path, &result, cp)?;
        }
        Format::Jsonl => {
            write_jsonl(output_path, &result)?;
        }
    }

    mlog!(
        "{} export: exported {} rows to {}",
        fmt.as_str(),
        result.rows.len(),
        output_path
    );

    Ok(ExportResult {
        path: output_path.to_string(),
        rows: result.rows.len(),
        columns: result.columns.len(),
    })
}

/// Writes a `QueryResult` to a CSV file (header + rows).
///
/// Compression is determined by the file extension (see
/// [`crate::format::file_compression_from_path`]).
fn write_csv(path: &str, result: &QueryResult, codepage: CodePage) -> Result<(), String> {
    codepage.validate_for(Format::Csv)?;

    // Build CSV content as UTF-8, then encode to the target codepage
    let mut csv_content = String::new();
    let header: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    csv_content.push_str(&header.join(","));
    csv_content.push('\n');

    for row in &result.rows {
        let fields: Vec<String> = row.iter().map(|v| value_to_text(v)).collect();
        // Simple CSV escaping: quote fields containing commas, quotes, or newlines
        let escaped: Vec<String> = fields
            .iter()
            .map(|f| {
                if f.contains(',') || f.contains('"') || f.contains('\n') {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f.clone()
                }
            })
            .collect();
        csv_content.push_str(&escaped.join(","));
        csv_content.push('\n');
    }

    // Encode to target codepage
    let encoded = codepage.encode(&csv_content);

    // Write (with compression if extension indicates)
    let mut sink = TextWriter::create(path)?;
    sink.write_bytes(&encoded)?;
    let inner = sink.into_inner();
    TextWriter::finish(inner)?;
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
        DataType::Decimal128(p, s) => {
            let scale = *s;
            let arr: Decimal128Array = values
                .iter()
                .map(|v| match v {
                    Value::Null => None,
                    Value::Text(txt) => parse_decimal(txt, scale),
                    Value::Int(n) => Some(*n as i128 * 10i128.pow(scale.max(0) as u32)),
                    _ => None,
                })
                .collect();
            // The FromIterator impl uses the max Decimal128 type (38, 10);
            // override with the schema's precision/scale so the array matches.
            let arr = arr.with_data_type(DataType::Decimal128(*p, *s));
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

/// Parses a decimal string (e.g. "12.3456") into an unscaled i128 for the
/// given scale.
///
/// The fractional part is right-padded (or truncated) to exactly `scale`
/// digits, so the result is `int_part * 10^scale + frac_part` — e.g.
/// `"80.00"` at scale 2 → `8000`, `"12.3"` at scale 4 → `123000`.
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
        let scale = scale as u32;
        int_part * 10i128.pow(scale)
            + frac_part
                * 10i128.pow(scale.saturating_sub(frac_digits(parts.get(1).map_or("", |v| v))))
    };
    Some(if negative { -unscaled } else { unscaled })
}

/// Number of digits in a fractional part (empty string → 0).
fn frac_digits(frac: &str) -> u32 {
    frac.chars().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decimal_positive_scale() {
        assert_eq!(parse_decimal("80.00", 2), Some(8000));
        assert_eq!(parse_decimal("12.3456", 4), Some(123456));
        assert_eq!(parse_decimal("12.3", 4), Some(123000));
        assert_eq!(parse_decimal("0.10", 2), Some(10));
        assert_eq!(parse_decimal("-1234.5678", 4), Some(-12345678));
        assert_eq!(parse_decimal("7", 2), Some(700));
        assert_eq!(parse_decimal("7.5", 2), Some(750));
    }

    #[test]
    fn parse_decimal_negative_scale() {
        assert_eq!(parse_decimal("123", -2), Some(12300));
        assert_eq!(parse_decimal("-123", -2), Some(-12300));
    }

    #[test]
    fn parse_decimal_invalid() {
        assert_eq!(parse_decimal("abc", 2), None);
    }

    #[test]
    fn decimal128_array_preserves_scale() {
        // Reproduces the Parquet export path: build a Decimal128Array from
        // decimal text values and verify the array's scale matches the schema.
        let v0 = Value::Text("1234.5678".into());
        let v1 = Value::Text("80.0000".into());
        let v2 = Value::Text("-0.0001".into());
        let v3 = Value::Text("99999999.9999".into());
        let values: Vec<&Value> = vec![&v0, &v1, &v2, &v3];
        let arr = build_column_array(&DataType::Decimal128(12, 4), &values, 4).unwrap();
        let dec = arr
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .expect("expected Decimal128Array");
        use arrow::array::Array;
        assert_eq!(dec.data_type(), &DataType::Decimal128(12, 4));
        assert_eq!(dec.value(0), 12345678);
        assert_eq!(dec.value(1), 800000);
        assert_eq!(dec.value(2), -1);
        assert_eq!(dec.value(3), 999999999999);
    }
}

/// Writes a `RecordBatch` to a Parquet file.
///
/// `compression` overrides the default (Snappy): `"snappy"`, `"zstd"`, `"lz4"`, or `"none"`.
fn write_parquet(
    path: &str,
    schema: &Schema,
    batch: &RecordBatch,
    compression: Option<&str>,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::BufWriter;

    crate::io::create_parent_dir(path)?;
    let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
    let writer = BufWriter::new(file);

    let codec = match compression {
        None | Some("snappy") => Compression::SNAPPY,
        Some("zstd") => Compression::ZSTD(Default::default()),
        Some("lz4") => Compression::LZ4,
        Some("none") => Compression::UNCOMPRESSED,
        Some(other) => {
            return Err(format!(
                "unsupported compression '{other}' for Parquet — use 'snappy', 'zstd', 'lz4' or 'none'"
            ));
        }
    };

    let props = WriterProperties::builder().set_compression(codec).build();

    let mut writer = ArrowWriter::try_new(writer, Arc::new(schema.clone()), Some(props))
        .map_err(|e| format!("cannot open Parquet writer: {e}"))?;
    writer
        .write(batch)
        .map_err(|e| format!("cannot write Parquet batch: {e}"))?;
    writer
        .close()
        .map_err(|e| format!("cannot close Parquet file: {e}"))?;
    Ok(())
}

/// Writes a `RecordBatch` to an Arrow IPC file (Feather v2).
///
/// `compression` overrides the default (ZSTD): `"zstd"`, `"lz4"`, or `"none"`.
fn write_ipc(
    path: &str,
    schema: &Schema,
    batch: &RecordBatch,
    compression: Option<&str>,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::BufWriter;

    crate::io::create_parent_dir(path)?;
    let file = File::create(path).map_err(|e| format!("cannot create {path}: {e}"))?;
    let writer = BufWriter::new(file);

    let compression_type = match compression {
        None | Some("zstd") => Some(CompressionType::ZSTD),
        Some("lz4") => Some(CompressionType::LZ4_FRAME),
        Some("none") => None,
        Some(other) => {
            return Err(format!(
                "unsupported compression '{other}' for IPC — use 'zstd', 'lz4' or 'none'"
            ));
        }
    };

    let options = IpcWriteOptions::default()
        .try_with_compression(compression_type)
        .map_err(|e| format!("cannot configure IPC compression: {e}"))?;
    let mut writer = IpcFileWriter::try_new_with_options(writer, schema, options)
        .map_err(|e| format!("cannot open IPC writer: {e}"))?;
    writer
        .write(batch)
        .map_err(|e| format!("cannot write IPC batch: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("cannot close IPC file: {e}"))?;
    Ok(())
}
