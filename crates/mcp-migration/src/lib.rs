//! # firebird-5-mcp-migration
//!
//! Data file export/import for Firebird databases.
//!
//! Supports Parquet (via Apache Arrow) and CSV. The public API is
//! format-agnostic: `export()`, `import()`, and `describe_file()` take a
//! `format` parameter and dispatch to the format-specific implementation.
//!
//! ## Export
//!
//! - [`export`] — exports a table, view, or raw SQL query to a data file.
//!   All three sources go through the same engine (unified path).
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
//!
//! ## Describe
//!
//! - [`describe_file`] — reads a file's schema without loading all data.
//!   - Parquet: column names, Arrow types, Firebird types, row count.
//!   - CSV: column names + first 5 sample rows (CSV is untyped).
//!   No database connection needed.
//!
//! ## Type mapping
//!
//! See [`types`] for the full Firebird ↔ Arrow type mapping.

pub mod csv;
pub mod describe;
pub mod export;
pub mod import;
pub mod types;

pub use csv::import_csv;
pub use describe::{FileDescription, describe_file};
pub use export::{ExportResult, export};
pub use import::{ImportMode, ImportResult, import_parquet};

/// Imports a data file into a Firebird table.
///
/// Dispatches on `format`: `"parquet"` or `"csv"`.
pub fn import(
    client: &mut firebird_5_mcp_core::client::FirebirdClient,
    input_path: &str,
    table: &str,
    mode: ImportMode,
    format: &str,
    ignore_columns: &[String],
) -> Result<ImportResult, String> {
    match format.to_lowercase().as_str() {
        "parquet" => import_parquet(client, input_path, table, mode, ignore_columns),
        "csv" => import_csv(client, input_path, table, mode, ignore_columns),
        other => Err(format!(
            "unsupported format '{other}' — only 'parquet' and 'csv' are supported"
        )),
    }
}
