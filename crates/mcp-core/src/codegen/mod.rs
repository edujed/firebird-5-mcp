//! Code generation for development artifacts from Firebird schemas.
//!
//! This module is the single entry point for all code generation in
//! `mcp-core`. It is split into:
//!
//! - [`fbtype`] — the typed [`FbType`](fbtype::FbType) enum (replaces the old
//!   `ResolvedType` bag of ints + `is_*` predicates)
//! - [`naming`] — shared naming helpers (`to_pascal_case`, `to_snake_case`,
//!   `to_camel_case`)
//! - [`generator`] — the dispatcher: [`Language`], [`GenOptions`], and the
//!   public [`generate`](generator::generate) entry point
//! - [`targets`] — one module per language, each implementing the
//!   [`Target`](targets::Target) trait
//! - [`view_dto`] — view DTO generation (resolved metadata via
//!   `SELECT FIRST 1 * FROM view`)
//! - [`ui`] — UI/form generation targets (Lazarus `.lfm` forms, Svelte 5
//!   pages) in a separate sub-module from the data targets
//!
//! The public API is re-exported here so callers can use
//! `codegen::generate`, `codegen::Language`, etc. without importing each
//! sub-module individually.

pub mod fbtype;
pub mod generator;
pub mod naming;
pub mod targets;
pub mod ui;
pub mod view_dto;

use crate::schema::ColumnInfo;
use mcp_types::is_not_null;

pub use fbtype::FbType;
pub use generator::{GenOptions, Language, generate};
pub use naming::{to_camel_case, to_pascal_case, to_snake_case};
pub use targets::Target;

/// Returns `true` if the column is nullable.
///
/// `RDB$NULL_FLAG`: `1` = NOT NULL, `NULL`/`0` = nullable.
/// This is the single source of truth for nullability across all codegen targets.
pub fn is_nullable(col: &ColumnInfo) -> bool {
    !is_not_null(col.nullable)
}
pub use ui::lazarus::{
    LazarusField, from_column_info as lazarus_from_column_info,
    from_view_field as lazarus_from_view_field, generate_lfm,
};
pub use ui::svelte::{
    SvelteField, from_column_info as svelte_from_column_info,
    from_view_field as svelte_from_view_field, generate_form, generate_page,
    generate_page_for_view,
};
pub use view_dto::{ViewField, generate_view_dto, get_view_metadata};
