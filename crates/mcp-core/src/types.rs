//! Firebird type formatting and identifier validation.
//!
//! These helpers now live in the [`firebird_5_mcp_types`] crate so that
//! sibling crates (e.g. the OpenAI-API edition) can share them without
//! depending on the full driver stack. This module re-exports them for
//! backward compatibility.

pub use firebird_5_mcp_types::{
    ColumnMeta, QueryResult, Value, format_cell, format_column_type, is_valid_identifier,
};
