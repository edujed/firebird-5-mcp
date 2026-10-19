//! # firebird-5-mcp-migration
//!
//! Data file export/import for Firebird databases.
//!
//! Supports Parquet, Arrow IPC (Feather v2), CSV, and DBF (dBASE/FoxPro/Clipper).
//! The public API is format-agnostic: `export()`, `import()`, and `describe_file()`
//! take a `format` parameter and dispatch to the format-specific implementation.
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
//!   - DBF: schema comes from the file header (no inference needed).
//!   - Parquet/IPC: compression is auto-detected from the file header.
//!   - CSV/DBF: compression is auto-detected from the file extension.
//!
//! ## Describe
//!
//! - [`describe_file`] — reads a file's schema without loading all data.
//!   - Parquet/IPC: column names, Arrow types, Firebird types, row count.
//!   - CSV: column names + first 5 sample rows (CSV is untyped).
//!   - DBF: column names, DBF types, Firebird types, row count.
//!   No database connection needed.
//!
//! ## Module layout
//!
//! - [`format`] — `Format` and `Compression` enums + codec detection
//! - [`io`] — compression-aware file I/O (`TextReader`/`TextWriter`)
//! - [`types`] — Firebird ↔ Arrow ↔ DBF type mapping
//! - [`value`] — Firebird `Value` → primitive conversion helpers
//! - [`arrow_json`] — Arrow array → `serde_json::Value` conversion
//! - [`table`] — table existence check, creation, append verification
//! - [`export`] — export engine (query → Arrow → file)
//! - [`import`] — import engine (file → Arrow → table) + shared batch helpers
//! - [`csv`] — CSV import + type inference
//! - [`dbf`] — DBF (dBASE/FoxPro/Clipper) import, export, describe
//! - [`describe`] — file schema description

pub mod arrow_json;
pub mod csv;
pub mod dbf;
pub mod describe;
pub mod export;
pub mod format;
pub mod import;
pub mod io;
pub mod jsonl;
pub mod table;
pub mod types;
pub mod value;

pub use csv::import_csv;
pub use dbf::{describe_dbase, import_dbase, write_dbase};
pub use describe::{FileDescription, describe_file};
pub use export::{ExportResult, export};
pub use format::{CodePage, Compression, Format, file_compression_from_path};
pub use import::{ImportMode, ImportResult, import_ipc, import_parquet};
pub use io::{TextReader, TextWriter};
pub use jsonl::{describe_jsonl, import_jsonl, write_jsonl};

/// Imports a data file into a Firebird table.
///
/// Dispatches on `format`: `"parquet"`, `"ipc"`, `"csv"`, `"dbf"`, or `"jsonl"`.
pub fn import(
    client: &mut firebird_5_mcp_core::client::FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    format: &str,
    ignore_columns: &[String],
    codepage: Option<&str>,
) -> Result<ImportResult, String> {
    let fmt = Format::parse(format)?;
    let cp = match codepage {
        Some(s) => CodePage::parse(s)?,
        None => CodePage::default_for(fmt),
    };
    match fmt {
        Format::Parquet => import_parquet(client, input_path, table, mode, ignore_columns),
        Format::Ipc => import_ipc(client, input_path, table, mode, ignore_columns),
        Format::Csv => import_csv(client, input_path, table, mode, ignore_columns, cp),
        Format::Dbase => import_dbase(client, input_path, table, mode, ignore_columns, cp),
        Format::Jsonl => import_jsonl(client, input_path, table, mode, ignore_columns),
    }
}
