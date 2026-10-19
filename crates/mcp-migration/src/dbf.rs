//! DBF (dBASE / FoxPro / Clipper) import, export, and describe.
//!
//! DBF files have a fixed schema defined in the file header (column names,
//! types, and lengths), so no type inference is needed on import — unlike CSV.
//!
//! DBF files predate UTF-8 — they use DOS code pages (CP850/CP437).
//! We read with CP850 (most common for Western languages, covers Portuguese).
//!
//! Compression is determined by file extension (same pattern as CSV):
//! `.dbf` (none), `.dbf.zst` (ZSTD), `.dbf.gz` (GZIP), `.dbf.lz4` (LZ4).

use std::io::Cursor;

use chrono::{Datelike, Timelike};
use dbase::{Date, DateTime, FieldName, FieldValue, Record, TableWriterBuilder, Time};

use firebird_5_mcp_core::client::FirebirdClient;
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_core::query::Value;
use firebird_5_mcp_types::QueryResult;

use crate::format::{CodePage, Format};
use crate::import::{ImportMode, ImportResult, insert_rows_in_batches, keep_column_indices};
use crate::io::{TextReader, TextWriter};
use crate::table::{create_table_from_types, verify_append_types};
use crate::types::{column_meta_to_arrow, dbf_type_to_firebird};
use crate::value::{bool_or_false, float_or_zero, value_to_text};

// --- Type mapping: Arrow DataType → DBF field (for export) ---

/// Determines the DBF field type + length + decimal for an Arrow DataType.
fn arrow_to_dbf_field(dt: &arrow::datatypes::DataType) -> (dbase::FieldType, u8, u8) {
    use arrow::datatypes::DataType::*;
    match dt {
        Boolean => (dbase::FieldType::Logical, 1, 0),
        Int16 => (dbase::FieldType::Numeric, 6, 0),
        Int32 => (dbase::FieldType::Numeric, 11, 0),
        Int64 => (dbase::FieldType::Numeric, 15, 0),
        Float32 => (dbase::FieldType::Float, 12, 4),
        Float64 => (dbase::FieldType::Numeric, 18, 6),
        Utf8 | LargeUtf8 => (dbase::FieldType::Character, 254, 0),
        Binary | FixedSizeBinary(_) => (dbase::FieldType::Character, 254, 0),
        Date32 | Date64 => (dbase::FieldType::Date, 8, 0),
        Timestamp(_, _) => (dbase::FieldType::DateTime, 8, 0),
        Time32(_) | Time64(_) => (dbase::FieldType::Character, 8, 0),
        Decimal128(p, s) => (dbase::FieldType::Numeric, (*p).min(15) as u8, *s as u8),
        _ => (dbase::FieldType::Character, 254, 0),
    }
}

// --- Import ---

/// Imports a DBF file into a Firebird table.
///
/// The DBF file header defines the column schema (names, types, lengths),
/// so no type inference is needed. Compression is auto-detected from the
/// file extension.
pub fn import_dbase(
    client: &mut FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
    codepage: CodePage,
) -> Result<ImportResult, String> {
    codepage.validate_for(Format::Dbase)?;
    let data = TextReader::open(input_path)?.into_bytes()?;
    let mut reader = match codepage {
        CodePage::Cp437 => dbase::ReaderBuilder::new()
            .with_encoding(dbase::yore::code_pages::CP437)
            .build(Cursor::new(data))
            .map_err(|e| format!("cannot parse DBF file: {e}"))?,
        _ => dbase::ReaderBuilder::new()
            .with_encoding(dbase::yore::code_pages::CP850)
            .build(Cursor::new(data))
            .map_err(|e| format!("cannot parse DBF file: {e}"))?,
    };

    // Get field info from the header (owned — fields() borrows the reader)
    let fields: Vec<dbase::FieldInfo> = reader.fields().to_vec();
    let num_records = reader.header().num_records as usize;

    if num_records == 0 {
        return Err("DBF file contains no records".to_string());
    }

    // Determine which columns to keep
    let keep_indices = keep_column_indices(fields.len(), |i| fields[i].name(), ignore_columns);

    if keep_indices.is_empty() {
        return Err("all columns are in ignore_columns — nothing to import".to_string());
    }

    // Build column names + Firebird types for kept columns
    let columns: Vec<String> = keep_indices
        .iter()
        .map(|&i| fields[i].name().trim().to_string())
        .collect();
    let types: Vec<String> = keep_indices
        .iter()
        .map(|&i| dbf_type_to_firebird(&fields[i].field_type(), fields[i].length()))
        .collect();

    // Create or verify table
    match mode {
        ImportMode::Create => {
            create_table_from_types(client, table, &columns, &types, &[])?;
        }
        ImportMode::Append => {
            verify_append_types(client, table, &columns)?;
        }
    }

    // Read records and insert in batches
    let rows = reader.iter_records().map(|record_result| {
        let record = record_result.map_err(|e| format!("cannot read DBF record: {e}"))?;
        let mut row = Vec::with_capacity(keep_indices.len());
        for &col_idx in &keep_indices {
            let field_name = fields[col_idx].name().trim().to_string();
            let value = record.get(&field_name);
            row.push(dbf_value_to_json(value));
        }
        Ok(row)
    });

    let inserted = insert_rows_in_batches(client, table, &columns, rows)?;

    mlog!("DBF import: imported {inserted} rows into {table}");

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: keep_indices.len(),
        mode,
    })
}

/// Converts a DBF `FieldValue` to a `serde_json::Value` for `insert_batch`.
fn dbf_value_to_json(value: Option<&FieldValue>) -> serde_json::Value {
    match value {
        None => serde_json::Value::Null,
        Some(FieldValue::Character(None)) => serde_json::Value::Null,
        Some(FieldValue::Character(Some(s))) => serde_json::Value::String(s.trim().to_string()),
        Some(FieldValue::Numeric(None)) => serde_json::Value::Null,
        Some(FieldValue::Numeric(Some(n))) => serde_json::json!(n),
        Some(FieldValue::Logical(None)) => serde_json::Value::Null,
        Some(FieldValue::Logical(Some(b))) => serde_json::Value::Bool(*b),
        Some(FieldValue::Date(None)) => serde_json::Value::Null,
        Some(FieldValue::Date(Some(d))) => serde_json::Value::String(format_date(d)),
        Some(FieldValue::Float(None)) => serde_json::Value::Null,
        Some(FieldValue::Float(Some(f))) => serde_json::json!(f),
        Some(FieldValue::Integer(i)) => serde_json::json!(i),
        Some(FieldValue::Currency(c)) => serde_json::json!(c),
        Some(FieldValue::DateTime(dt)) => serde_json::Value::String(format_datetime(dt)),
        Some(FieldValue::Double(d)) => serde_json::json!(d),
        Some(FieldValue::Memo(m)) => serde_json::Value::String(m.clone()),
    }
}

/// Formats a DBF `Date` as "YYYY-MM-DD".
fn format_date(d: &Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day())
}

/// Formats a DBF `DateTime` as "YYYY-MM-DD HH:MM:SS".
fn format_datetime(dt: &DateTime) -> String {
    let date = dt.date();
    let time = dt.time();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.year(),
        date.month(),
        date.day(),
        time.hours(),
        time.minutes(),
        time.seconds()
    )
}

// --- Export ---

/// Writes a `QueryResult` to a DBF file.
///
/// Compression is determined by the file extension (`.dbf`, `.dbf.zst`,
/// `.dbf.gz`, `.dbf.lz4`).
pub fn write_dbase(path: &str, result: &QueryResult, codepage: CodePage) -> Result<(), String> {
    codepage.validate_for(Format::Dbase)?;
    let schema = column_meta_to_arrow(&result.columns);

    // DBF field names are limited to 10 characters (dBASE 3/4/Clipper legacy rule).
    // The 11th byte of the field descriptor is reserved for the field type.
    // Abort with a clear error listing the offending columns instead of
    // silently truncating — truncation breaks column mapping in legacy integrations.
    let too_long: Vec<&str> = schema
        .fields()
        .iter()
        .filter(|f| f.name().trim().len() > 10)
        .map(|f| f.name().trim())
        .collect();
    if !too_long.is_empty() {
        return Err(format!(
            "DBF field names cannot exceed 10 characters (dBASE 3/4/Clipper limit). Offending columns: {}. \
             Use shorter column names or add aliases in your SELECT (e.g. SELECT DEPARTAMENTO_NOME AS DEPTO).",
            too_long.join(", ")
        ));
    }

    // Write to a temp file first (TableWriter needs Write + Seek)
    let tmp_path = format!("{path}.tmp");

    let mut builder = match codepage {
        CodePage::Cp437 => TableWriterBuilder::with_encoding(dbase::yore::code_pages::CP437),
        _ => TableWriterBuilder::with_encoding(dbase::yore::code_pages::CP850),
    };
    for field in schema.fields() {
        let name = field.name().trim();
        let (ft, length, decimal) = arrow_to_dbf_field(field.data_type());
        let field_name =
            FieldName::try_from(name).map_err(|e| format!("invalid field name: {e}"))?;
        builder = match ft {
            dbase::FieldType::Character => builder.add_character_field(field_name, length),
            dbase::FieldType::Numeric => builder.add_numeric_field(field_name, length, decimal),
            dbase::FieldType::Float => builder.add_float_field(field_name, length, decimal),
            dbase::FieldType::Logical => builder.add_logical_field(field_name),
            dbase::FieldType::Date => builder.add_date_field(field_name),
            dbase::FieldType::DateTime => builder.add_datetime_field(field_name),
            dbase::FieldType::Integer => builder.add_integer_field(field_name),
            dbase::FieldType::Double => builder.add_double_field(field_name),
            dbase::FieldType::Currency => builder.add_currency_field(field_name),
            dbase::FieldType::Memo => builder.add_character_field(field_name, 255),
        };
    }

    let mut writer = builder
        .build_with_file_dest(&tmp_path)
        .map_err(|e| format!("cannot open DBF writer: {e}"))?;

    for row in &result.rows {
        let mut record = Record::default();
        for (col_idx, field) in schema.fields().iter().enumerate() {
            let value = row.get(col_idx).unwrap_or(&Value::Null);
            let fv = value_to_dbf_field(value, field.data_type());
            record.insert(field.name().trim().to_string(), fv);
        }
        writer
            .write_record(&record)
            .map_err(|e| format!("cannot write DBF record: {e}"))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("cannot finalize DBF writer: {e}"))?;

    // Read the written bytes from the temp file, then write (compressed) to the final path
    let mut data = std::fs::read(&tmp_path).map_err(|e| format!("cannot read temp DBF: {e}"))?;
    let _ = std::fs::remove_file(&tmp_path);

    // DEBUG: force-append the 0x1A terminator to test if it fixes the file
    if !data.ends_with(&[0x1A]) {
        mlog!(
            "DBF export: 0x1A terminator missing from dbase output, appending (size before: {})",
            data.len()
        );
        data.push(0x1A);
    }

    let mut sink = TextWriter::create(path)?;
    sink.write_bytes(&data)?;
    let inner = sink.into_inner();
    TextWriter::finish(inner)?;

    Ok(())
}

/// Converts a Firebird `Value` to a DBF `FieldValue` based on the expected
/// Arrow data type.
fn value_to_dbf_field(value: &Value, dt: &arrow::datatypes::DataType) -> FieldValue {
    use arrow::datatypes::DataType;
    match dt {
        DataType::Int16 | DataType::Int32 | DataType::Int64 => {
            FieldValue::Numeric(Some(float_or_zero(value)))
        }
        DataType::Float32 | DataType::Float64 => FieldValue::Numeric(Some(float_or_zero(value))),
        DataType::Utf8 => {
            let s = value_to_text(value);
            let s: String = s.chars().take(254).collect();
            FieldValue::Character(Some(s))
        }
        DataType::Boolean => FieldValue::Logical(Some(bool_or_false(value))),
        DataType::Date32 => {
            if let Value::Date(d) = value {
                Date::new(d.day() as u32, d.month() as u32, d.year() as u32)
                    .map(|d| FieldValue::Date(Some(d)))
                    .unwrap_or(FieldValue::Date(None))
            } else {
                FieldValue::Date(None)
            }
        }
        DataType::Timestamp(_, _) => {
            if let Value::Timestamp(ts) = value {
                let date = Date::new(ts.day() as u32, ts.month() as u32, ts.year() as u32)
                    .unwrap_or_else(|_| Date::new(1, 1, 1900).unwrap());
                let time = Time::new(ts.hour() as u32, ts.minute() as u32, ts.second() as u32)
                    .unwrap_or_else(|_| Time::new(0, 0, 0).unwrap());
                FieldValue::DateTime(DateTime::new(date, time))
            } else {
                let date = Date::new(1, 1, 1900).unwrap();
                let time = Time::new(0, 0, 0).unwrap();
                FieldValue::DateTime(DateTime::new(date, time))
            }
        }
        DataType::Time64(_) => {
            let s = value_to_text(value);
            let s: String = s.chars().take(254).collect();
            FieldValue::Character(Some(s))
        }
        DataType::Binary => {
            if let Value::Binary(data) = value {
                let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
                let hex: String = hex.chars().take(254).collect();
                FieldValue::Character(Some(hex))
            } else {
                FieldValue::Character(Some(String::new()))
            }
        }
        DataType::Decimal128(_, _) => FieldValue::Numeric(Some(float_or_zero(value))),
        _ => {
            let s = value_to_text(value);
            let s: String = s.chars().take(254).collect();
            FieldValue::Character(Some(s))
        }
    }
}

// --- Describe ---

/// Describes a DBF file's schema without loading all records.
pub fn describe_dbase(
    path: &str,
    codepage: CodePage,
) -> Result<crate::describe::FileDescription, String> {
    codepage.validate_for(Format::Dbase)?;
    let data = TextReader::open(path)?.into_bytes()?;
    let reader = match codepage {
        CodePage::Cp437 => dbase::ReaderBuilder::new()
            .with_encoding(dbase::yore::code_pages::CP437)
            .build(Cursor::new(data))
            .map_err(|e| format!("cannot parse DBF file: {e}"))?,
        _ => dbase::ReaderBuilder::new()
            .with_encoding(dbase::yore::code_pages::CP850)
            .build(Cursor::new(data))
            .map_err(|e| format!("cannot parse DBF file: {e}"))?,
    };

    let fields = reader.fields();
    let row_count = reader.header().num_records as i64;

    let mut columns = Vec::with_capacity(fields.len());
    let mut arrow_types = Vec::with_capacity(fields.len());
    let mut firebird_types = Vec::with_capacity(fields.len());

    for field in fields {
        columns.push(field.name().trim().to_string());
        arrow_types.push(format!("{:?}", field.field_type()));
        firebird_types.push(dbf_type_to_firebird(&field.field_type(), field.length()));
    }

    Ok(crate::describe::FileDescription {
        path: path.to_string(),
        format: Format::Dbase.as_str().to_string(),
        rows: row_count,
        columns,
        arrow_types,
        firebird_types,
        sample_rows: Vec::new(),
    })
}
