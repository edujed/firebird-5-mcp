//! Describes the schema of a data file without loading its data.
//!
//! For Parquet/IPC: reads the file footer to get column names, Arrow types,
//! mapped Firebird types, and row count. No database connection needed.
//!
//! For CSV: reads the header line and the first 5 data rows as a sample
//! (CSV is untyped, so we show actual values instead of types).

use arrow::ipc::reader::FileReader as IpcFileReader;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;

use crate::dbf::describe_dbase;
use crate::format::{CodePage, Format};
use crate::io::TextReader;
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
    /// Type names (Arrow types for Parquet/IPC; "inferred" for CSV).
    pub arrow_types: Vec<String>,
    /// Mapped Firebird DDL types (for Parquet/IPC; empty for CSV).
    pub firebird_types: Vec<String>,
    /// Sample rows (first N data rows). Empty for Parquet/IPC.
    pub sample_rows: Vec<Vec<String>>,
}

/// Describes a data file's schema.
pub fn describe_file(
    path: &str,
    format: &str,
    codepage: Option<&str>,
) -> Result<FileDescription, String> {
    let fmt = Format::parse(format)?;
    let cp = match codepage {
        Some(s) => CodePage::parse(s)?,
        None => CodePage::default_for(fmt),
    };
    match fmt {
        Format::Parquet => describe_parquet(path),
        Format::Ipc => describe_ipc(path),
        Format::Csv => describe_csv(path, cp),
        Format::Dbase => describe_dbase(path, cp),
    }
}

fn describe_parquet(path: &str) -> Result<FileDescription, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("cannot read Parquet file: {e}"))?;

    let schema = (**builder.schema()).clone();
    let row_count = builder.metadata().file_metadata().num_rows();

    let (columns, arrow_types, firebird_types) = schema_to_description(&schema);

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

fn describe_ipc(path: &str) -> Result<FileDescription, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let mut reader =
        IpcFileReader::try_new(file, None).map_err(|e| format!("cannot read IPC file: {e}"))?;

    let schema = reader.schema().clone();
    let row_count: i64 = reader
        .by_ref()
        .map(|b| b.map(|b| b.num_rows() as i64).unwrap_or(0))
        .sum();

    let (columns, arrow_types, firebird_types) = schema_to_description(&schema);

    Ok(FileDescription {
        path: path.to_string(),
        format: "ipc".to_string(),
        rows: row_count,
        columns,
        arrow_types,
        firebird_types,
        sample_rows: Vec::new(),
    })
}

fn describe_csv(path: &str, codepage: CodePage) -> Result<FileDescription, String> {
    codepage.validate_for(Format::Csv)?;

    // Read raw bytes, decompress, then decode with the specified codepage
    let raw_bytes = TextReader::open(path)?.into_bytes()?;
    let csv_text = codepage.decode(&raw_bytes);
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());

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

/// Builds the (columns, arrow_types, firebird_types) triple from an Arrow schema.
fn schema_to_description(
    schema: &arrow::datatypes::Schema,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut columns = Vec::with_capacity(schema.fields().len());
    let mut arrow_types = Vec::with_capacity(schema.fields().len());
    let mut firebird_types = Vec::with_capacity(schema.fields().len());

    for field in schema.fields() {
        columns.push(field.name().clone());
        arrow_types.push(format!("{}", field.data_type()));
        firebird_types.push(arrow_to_firebird(field.data_type()));
    }

    (columns, arrow_types, firebird_types)
}
