//! Firebird type formatting and identifier validation.
//!
//! The shared types (`Value`, `ColumnMeta`, `QueryResult`, `format_cell`,
//! `is_valid_identifier`, `value_to_json`) come from `mcp-types`.
//! The Firebird-specific helpers (`format_column_type`, `is_not_null`,
//! `trigger_type_label`) are defined in `mcp-types` as well.

pub use mcp_types::{
    ColumnMeta, QueryResult, Value, format_cell, format_column_type, is_not_null,
    is_valid_identifier, trigger_type_label, value_to_json,
};
