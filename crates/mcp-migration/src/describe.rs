//! Describes the schema of a data file without loading its data.
//!
//! For Parquet: reads the file footer to get column names, Arrow types,
//! mapped Firebird types, and row count. No database connection needed.
//!
//! For CSV: reads the header line and the first 5 data rows as a sample
//! (CSV is untyped, so we show actual values instead of types).

use std::fs::File;

use csv::ReaderBuilder;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::types::arrow_to_firebird;

/// Number of sample rows to show for CSV files.
const CSV_SAMPLE_ROWS: usize = 5;

/// Result of describing a data file.
#[derive(Debug)]
pub struct FileDescription {
    /// File path.
    pub path: String,
    /// File format (e.g. "parquet", "csv").
    pub format: String,
    /// Total row count (for CSV: approximate, excludes header).
    pub rows: i64,
    /// Column names.
    pub columns: Vec<String>,
    /// Type names (Arrow types for Parquet; "inferred" for CSV).
    pub arrow_types: Vec<String>,
    /// Mapped Firebird DDL types (for Parquet; empty for CSV).
    pub firebird_types: Vec<String>,
    /// Sample rows (first N data rows). Empty for Parquet.
    pub sample_rows: Vec<Vec<String>>,
}

/// Describes a data file's schema.
pub fn describe_file(path: &str, format: &str) -> Result<FileDescription, String> {
    match format.to_lowercase().as_str() {
        "parquet" => describe_parquet(path),
        "csv" => describe_csv(path),
        other => Err(format!(
            "unsupported format '{other}' — only 'parquet' and 'csv' are supported"
        )),
    }
}

fn describe_parquet(path: &str) -> Result<FileDescription, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("cannot read Parquet file: {e}"))?;

    let schema = (**builder.schema()).clone();
    let row_count = builder.metadata().file_metadata().num_rows();

    let mut columns = Vec::with_capacity(schema.fields().len());
    let mut arrow_types = Vec::with_capacity(schema.fields().len());
    let mut firebird_types = Vec::with_capacity(schema.fields().len());

    for field in schema.fields() {
        columns.push(field.name().clone());
        arrow_types.push(format!("{}", field.data_type()));
        firebird_types.push(arrow_to_firebird(field.data_type()));
    }

    Ok(FileDescription {
        path: path.to_string(),
        format: "parquet".to_string(),
        rows: row_count,
        columns,
        arrow_types,
        firebird_types,
        sample_rows: Vec::new(),
    })
}

fn describe_csv(path: &str) -> Result<FileDescription, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    // Read header
    let headers = reader
        .headers()
        .map_err(|e| format!("cannot read CSV header: {e}"))?
        .clone();
    let columns: Vec<String> = headers.iter().map(|h| h.trim().to_string()).collect();

    // Read sample rows + count total
    let mut sample_rows: Vec<Vec<String>> = Vec::with_capacity(CSV_SAMPLE_ROWS);
    let mut total_rows: i64 = 0;
    for result in reader.records() {
        let record = result.map_err(|e| format!("cannot read CSV row: {e}"))?;
        total_rows += 1;
        if sample_rows.len() < CSV_SAMPLE_ROWS {
            let fields: Vec<String> = (0..columns.len())
                .map(|i| record.get(i).unwrap_or("").trim().to_string())
                .collect();
            sample_rows.push(fields);
        }
    }

    let arrow_types: Vec<String> = columns.iter().map(|_| "inferred".to_string()).collect();
    let firebird_types: Vec<String> = vec![String::new(); columns.len()];

    Ok(FileDescription {
        path: path.to_string(),
        format: "csv".to_string(),
        rows: total_rows,
        columns,
        arrow_types,
        firebird_types,
        sample_rows,
    })
}
