//! Import engine: Parquet/IPC → Firebird table.
//!
//! Two modes:
//! - `create` — creates the table from the file schema, then inserts data.
//!   Fails if the table already exists.
//! - `append` — the table must already exist with a compatible structure.
//!   Data is inserted into the existing table.
//!
//! Parquet and IPC share the same import logic ([`import_record_batches`]);
//! the only difference is how the file is read.

use std::fs::File;

use arrow::array::RecordBatch;
use arrow::datatypes::{Field, Schema};
use arrow::ipc::reader::FileReader as IpcFileReader;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use firebird_5_mcp_core::{FirebirdClient, mlog};
use mcp_core::query::insert_batch;

use crate::arrow_json::arrow_value_to_json;
use crate::table::{create_table_from_arrow, verify_append};

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

/// Result of a file import.
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
    let file = File::open(input_path).map_err(|e| format!("cannot open {input_path}: {e}"))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| format!("cannot read Parquet file: {e}"))?;
    let full_schema: Schema = (**builder.schema()).clone();
    let batches: Vec<RecordBatch> = builder
        .build()
        .map_err(|e| format!("cannot build Parquet reader: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot read Parquet batches: {e}"))?;

    import_record_batches(
        client,
        &full_schema,
        &batches,
        table,
        mode,
        ignore_columns,
        "Parquet",
    )
}

/// Imports an Arrow IPC (Feather v2) file into a Firebird table.
///
/// Supports LZ4 and ZSTD compressed files (auto-detected from the file header).
pub fn import_ipc(
    client: &mut FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    let file = File::open(input_path).map_err(|e| format!("cannot open {input_path}: {e}"))?;
    let reader =
        IpcFileReader::try_new(file, None).map_err(|e| format!("cannot read IPC file: {e}"))?;
    let full_schema: Schema = (*reader.schema()).clone();
    let batches: Vec<RecordBatch> = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot read IPC batches: {e}"))?;

    import_record_batches(
        client,
        &full_schema,
        &batches,
        table,
        mode,
        ignore_columns,
        "IPC",
    )
}

/// Filters column indices, excluding those in `ignore_columns` (case-insensitive).
///
/// `name_at` returns the column name for a given index.
pub fn keep_column_indices<'a, F: Fn(usize) -> &'a str>(
    len: usize,
    name_at: F,
    ignore_columns: &[String],
) -> Vec<usize> {
    (0..len)
        .filter(|&i| {
            !ignore_columns
                .iter()
                .any(|ic| ic.to_uppercase() == name_at(i).to_uppercase())
        })
        .collect()
}

/// Inserts rows into a table in batches of 500 (single transaction per batch).
///
/// Shared by all import paths (Parquet, IPC, CSV, DBF).
pub fn insert_rows_in_batches(
    client: &mut FirebirdClient,
    table: &str,
    columns: &[String],
    rows: impl Iterator<Item = Result<Vec<serde_json::Value>, String>>,
) -> Result<usize, String> {
    const BATCH_SIZE: usize = 500;
    let mut inserted = 0;
    let mut batch: Vec<Vec<serde_json::Value>> = Vec::with_capacity(BATCH_SIZE);

    for row in rows {
        batch.push(row?);
        if batch.len() >= BATCH_SIZE {
            let n = insert_batch(client, table, columns, &batch).map_err(|e| e.to_string())?;
            inserted += n;
            batch.clear();
        }
    }

    if !batch.is_empty() {
        let n = insert_batch(client, table, columns, &batch).map_err(|e| e.to_string())?;
        inserted += n;
    }

    Ok(inserted)
}

/// Shared import logic: takes a schema + batches, applies `ignore_columns`,
/// creates/verifies the table, and inserts data.
fn import_record_batches(
    client: &mut FirebirdClient,
    full_schema: &Schema,
    batches: &[RecordBatch],
    table: &str,
    mode: ImportMode,
    ignore_columns: &[String],
    label: &str,
) -> Result<ImportResult, String> {
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    if total_rows == 0 {
        return Err(format!("{label} file contains no rows"));
    }

    // Determine which column indices to keep (exclude ignored columns)
    let keep_indices = keep_column_indices(
        full_schema.fields().len(),
        |i| full_schema.fields()[i].name(),
        ignore_columns,
    );

    if keep_indices.is_empty() {
        return Err("all columns are in ignore_columns — nothing to import".to_string());
    }

    // Build a filtered schema containing only the kept columns
    let filtered_fields: Vec<Field> = keep_indices
        .iter()
        .map(|&i| full_schema.field(i).clone())
        .collect();
    let schema = Schema::new(filtered_fields);

    match mode {
        ImportMode::Create => {
            create_table_from_arrow(client, table, &schema)?;
        }
        ImportMode::Append => {
            verify_append(client, table, &schema)?;
        }
    }

    // Insert data in batches (only kept columns)
    let columns: Vec<String> = keep_indices
        .iter()
        .map(|&i| full_schema.fields()[i].name().clone())
        .collect();
    let rows_iter = batches.iter().flat_map(|batch| {
        batch_to_rows_filtered(batch, full_schema, &keep_indices)
            .unwrap_or_default()
            .into_iter()
            .map(Ok)
    });
    let inserted = insert_rows_in_batches(client, table, &columns, rows_iter)?;

    mlog!("{label} import: imported {} rows into {}", inserted, table);

    Ok(ImportResult {
        table: table.to_string(),
        rows: inserted,
        columns: keep_indices.len(),
        mode,
    })
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
