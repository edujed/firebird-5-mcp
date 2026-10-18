//! # firebird-5-mcp-migration
//!
//! Data file export/import for Firebird databases.
//!
//! Supports Parquet, Arrow IPC (Feather v2), and CSV. The public API is
//! format-agnostic: `export()`, `import()`, and `describe_file()` take a
//! `format` parameter and dispatch to the format-specific implementation.
//!
//! ## Export
//!
//! - [`export`] — exports a table, view, or raw SQL query to a data file.
//!   All three sources go through the same engine (unified path).
//!   Compression is configurable per format (see [`format::Format`]).
//!
//! ## Import
//!
//! - [`import`] — imports a data file into a Firebird table.
//!   Two modes:
//!   - `create` — creates the table from the file schema, then inserts.
//!     Fails if the table already exists.
//!   - `append` — table must already exist with a compatible structure.
//!   - `ignore_columns` — optional list of file columns to skip.
//!   - CSV: column types are inferred from a sample of the first 100 rows.
//!   - Parquet/IPC: compression is auto-detected from the file header.
//!
//! ## Describe
//!
//! - [`describe_file`] — reads a file's schema without loading all data.
//!   - Parquet/IPC: column names, Arrow types, Firebird types, row count.
//!   - CSV: column names + first 5 sample rows (CSV is untyped).
//!   No database connection needed.
//!
//! ## Module layout
//!
//! - [`format`] — format/compression enums + CSV I/O (compression-aware)
//! - [`types`] — Firebird ↔ Arrow type mapping
//! - [`value`] — Firebird `Value` → primitive conversion helpers
//! - [`arrow_json`] — Arrow array → `serde_json::Value` conversion
//! - [`table`] — table existence check, creation, append verification
//! - [`export`] — export engine (query → Arrow → file)
//! - [`import`] — import engine (file → Arrow → table)
//! - [`csv`] — CSV import + type inference
//! - [`describe`] — file schema description

pub mod arrow_json;
pub mod csv;
pub mod describe;
pub mod export;
pub mod format;
pub mod import;
pub mod table;
pub mod types;
pub mod value;

pub use csv::import_csv;
pub use describe::{FileDescription, describe_file};
pub use export::{ExportResult, export};
pub use format::{Compression, CsvReader, CsvWriter, Format};
pub use import::{ImportMode, ImportResult, import_ipc, import_parquet};

/// Imports a data file into a Firebird table.
///
/// Dispatches on `format`: `"parquet"`, `"ipc"`, or `"csv"`.
pub fn import(
    client: &mut firebird_5_mcp_core::client::FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    format: &str,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    match Format::parse(format)? {
        Format::Parquet => import_parquet(client, input_path, table, mode, ignore_columns),
        Format::Ipc => import_ipc(client, input_path, table, mode, ignore_columns),
        Format::Csv => import_csv(client, input_path, table, mode, ignore_columns),
    }
}
