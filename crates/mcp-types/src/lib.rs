//! # firebird-5-mcp-types
//!
//! Shared value types for the Firebird MCP project.
//!
//! This crate holds the data shapes that are consumed by more than one
//! sibling crate (the stdio MCP server, and — in the future — the OpenAI-API
//! edition). It contains **no IO, no async, no driver code**: just value
//! types, result containers, and pure formatting helpers.
//!
//! ## What lives here
//!
//! - [`Value`] — a single column value returned by a query.
//! - [`ColumnMeta`] / [`QueryResult`] — the result of a SELECT statement.
//! - [`format_column_type`] — human-readable Firebird type descriptions.
//! - [`is_valid_identifier`] — SQL identifier validation.
//! - [`format_cell`] — Markdown cell rendering for result tables.
//!
//! ## What does not live here
//!
//! - Connection handling, query execution, schema introspection, DDL,
//!   codegen — those live in `firebird-5-mcp-core`.
//! - MCP protocol, tool dispatch, Markdown rendering of whole results —
//!   those live in `firebird-5-mcp` (the binary crate).

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};

/// A single column value returned by a query.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Date(NaiveDate),
    Time(NaiveTime),
    Timestamp(NaiveDateTime),
    Binary(Vec<u8>),
}

/// Metadata for a result column.
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    /// `true` when the column is a NUMERIC/DECIMAL (fixed-point) column.
    ///
    /// The vendor patch renders NUMERIC/DECIMAL as exact decimal text
    /// (e.g. `"80.00"`), but the `raw_type` stays `SQL_INT64` (580), so
    /// `type_name` alone cannot distinguish a NUMERIC from a real BIGINT.
    /// This flag is set by the query layer when it detects a scaled-integer
    /// column (raw_type 580 + decimal text value).
    pub is_numeric: bool,
}

/// The result of a SELECT statement.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
}

impl QueryResult {
    /// Returns the value at `row_index`/`col_index`, or Null if out of bounds.
    pub fn get(&self, row: usize, col: usize) -> &Value {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .unwrap_or(&Value::Null)
    }

    /// Looks up a column index by name (case-insensitive).
    pub fn col_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }
}

impl Value {
    /// Returns the value as an `i64`, parsing text and coercing booleans.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Text(s) => s.parse().ok(),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    /// Returns the value as a `String` (text or integer).
    pub fn as_text(&self) -> Option<String> {
        match self {
            Value::Text(s) => Some(s.clone()),
            Value::Int(n) => Some(n.to_string()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Firebird type formatting
// ---------------------------------------------------------------------------

/// Maps RDB$FIELD_TYPE numbers to Firebird type names.
/// Verified against RDB$FIELDS in Firebird 5.
const FIREBIRD_TYPE_NAMES: [(i64, &str); 24] = [
    (7, "SMALLINT"),
    (8, "INTEGER"),
    (9, "DATE"),
    (10, "FLOAT"),
    (11, "FLOAT"),
    (12, "DATE"),
    (13, "TIME"),
    (14, "CHAR"),
    (16, "BIGINT"),
    (20, "VARCHAR"),
    (23, "BOOLEAN"),
    (24, "DECFLOAT(16)"),
    (25, "DECFLOAT(34)"),
    (26, "INT128"),
    (27, "DOUBLE PRECISION"),
    (29, "TIMESTAMP"),
    (35, "TIMESTAMP"),
    (37, "VARCHAR"),
    (40, "CSTRING"),
    (43, "NUMERIC"),
    (45, "BLOB"),
    (48, "NVARCHAR"),
    (49, "BOOLEAN"),
    (261, "BLOB"),
];

fn type_name_for(type_num: i64) -> Option<&'static str> {
    FIREBIRD_TYPE_NAMES
        .iter()
        .find(|(n, _)| *n == type_num)
        .map(|(_, name)| *name)
}

/// Returns a human-readable type description for a column, taking into
/// account type number, sub-type, size (in bytes), precision, and scale.
///
/// For character types (CHAR, VARCHAR, NVARCHAR) the size is converted from
/// bytes to characters (UTF8 = 4 bytes/char, NVARCHAR = 2 bytes/char).
/// For NUMERIC/DECIMAL (identified by RDB$FIELD_SUB_TYPE), precision and
/// scale are included.
pub fn format_column_type(
    type_num: i64,
    sub_type: i64,
    size: i64,
    precision: i64,
    scale: i64,
) -> String {
    if type_num == 0 {
        return "UNKNOWN".to_string();
    }
    let mut type_name = match type_name_for(type_num) {
        Some(n) => n.to_string(),
        None => return format!("TYPE_{type_num}"),
    };

    // NUMERIC/DECIMAL detection via RDB$FIELD_SUB_TYPE.
    // Sub-type 1 = NUMERIC, 2 = DECIMAL.
    // NOTE: RDB$FIELD_SUB_TYPE is overloaded — for BLOB types (261) it means
    // blob sub-type, so only apply this check to integer base types.
    if (type_num == 7 || type_num == 8 || type_num == 16 || type_num == 26)
        && (sub_type == 1 || sub_type == 2)
    {
        type_name = if sub_type == 1 {
            "NUMERIC".to_string()
        } else {
            "DECIMAL".to_string()
        };
        if precision > 0 {
            let scale_abs = scale.abs();
            if scale_abs > 0 {
                return format!("{type_name}({precision},{scale_abs})");
            }
            return format!("{type_name}({precision})");
        }
    }

    // Add size info for character types
    match type_name.as_str() {
        "CHAR" | "VARCHAR" | "NVARCHAR" => {
            let char_size = char_size_for(size, &type_name);
            if char_size > 0 {
                return format!("{type_name}({char_size})");
            }
        }
        "BLOB" => {
            // For BLOB types (type_num 261), RDB$FIELD_SUB_TYPE indicates the
            // blob sub-type: 1 = TEXT, 2 = BINARY (per the Firebird docs and
            // RDB$FIELDS). 0/other = generic BLOB.
            if sub_type == 1 {
                return "BLOB SUB_TYPE TEXT".to_string();
            } else if sub_type == 2 {
                return "BLOB SUB_TYPE BINARY".to_string();
            }
            return "BLOB".to_string();
        }
        _ => {}
    }
    type_name
}

/// Returns the character count for a given character type.
///
/// In Firebird, `RDB$FIELD_LENGTH` for CHAR/VARCHAR/NVARCHAR is already in
/// **characters**, not bytes. The only exception is NVARCHAR, where the
/// length is in 2-byte units (so divide by 2 to get characters).
fn char_size_for(size: i64, type_name: &str) -> i64 {
    if size <= 0 {
        return 0;
    }
    match type_name {
        "NVARCHAR" => size / 2,
        _ => size, // CHAR, VARCHAR: already in characters
    }
}

// ---------------------------------------------------------------------------
// RDB$NULL_FLAG interpretation
// ---------------------------------------------------------------------------

/// Interprets `RDB$NULL_FLAG`: `1` = NOT NULL, `NULL`/`0` = nullable.
///
/// This is the single source of truth for NULL_FLAG semantics across the
/// entire project. Use this instead of comparing the raw value inline.
pub fn is_not_null(null_flag: Option<i64>) -> bool {
    null_flag == Some(1)
}

// ---------------------------------------------------------------------------
// RDB$TRIGGER_TYPE interpretation
// ---------------------------------------------------------------------------

/// Maps `RDB$TRIGGER_TYPE` to (timing, event) for DDL and display.
///
/// 1=BEFORE INSERT, 2=BEFORE UPDATE, 3=BEFORE DELETE,
/// 4=AFTER INSERT, 5=AFTER UPDATE, 6=AFTER DELETE.
pub fn trigger_type_label(trigger_type: i64) -> (&'static str, &'static str) {
    match trigger_type {
        1 => ("BEFORE", "INSERT"),
        2 => ("BEFORE", "UPDATE"),
        3 => ("BEFORE", "DELETE"),
        4 => ("AFTER", "INSERT"),
        5 => ("AFTER", "UPDATE"),
        6 => ("AFTER", "DELETE"),
        _ => ("BEFORE", "INSERT"),
    }
}

// ---------------------------------------------------------------------------
// Identifier validation
// ---------------------------------------------------------------------------

/// Checks if the name is a safe SQL identifier (letters, digits, underscore;
/// must start with a letter).
pub fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Markdown cell rendering
// ---------------------------------------------------------------------------

/// Renders a single cell value for a Markdown table: NULLs become "NULL",
/// newlines are flattened, and pipes are escaped.
pub fn format_cell(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format_float(*f),
        Value::Text(s) => flatten(s),
        Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        Value::Time(t) => t.format("%H:%M:%S").to_string(),
        Value::Timestamp(ts) => {
            if ts.nanosecond() != 0 {
                ts.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
            } else {
                ts.format("%Y-%m-%d %H:%M:%S").to_string()
            }
        }
        Value::Binary(_) => "<BLOB>".to_string(),
    }
}

fn format_float(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn flatten(s: &str) -> String {
    s.replace('\n', " ").replace('|', "\\|")
}

// ---------------------------------------------------------------------------
// JSON serialization (for structuredContent)
// ---------------------------------------------------------------------------

/// Serializes a [`Value`] to a [`serde_json::Value`], preserving type
/// fidelity for the Web UI's `structuredContent` payload.
///
/// - `Null` → `null`
/// - `Int` → JSON number
/// - `Float` → JSON number (or `null` if not finite)
/// - `Bool` → JSON boolean
/// - `Text` → JSON string (NUMERIC/DECIMAL arrives as exact decimal text
///   via the vendor patch and must stay a string to preserve precision)
/// - `Date` → `"YYYY-MM-DD"`
/// - `Time` → `"HH:MM:SS"`
/// - `Timestamp` → `"YYYY-MM-DD HH:MM:SS"` (with `.ffffff` when nanos ≠ 0)
/// - `Binary` → base64-encoded string
pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Int(n) => serde_json::Value::Number(serde_json::Number::from(*n)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::Date(d) => serde_json::Value::String(d.format("%Y-%m-%d").to_string()),
        Value::Time(t) => serde_json::Value::String(t.format("%H:%M:%S").to_string()),
        Value::Timestamp(ts) => {
            let s = if ts.nanosecond() != 0 {
                ts.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
            } else {
                ts.format("%Y-%m-%d %H:%M:%S").to_string()
            };
            serde_json::Value::String(s)
        }
        Value::Binary(data) => serde_json::Value::String(base64_encode(data)),
    }
}

/// Minimal base64 encoder (avoids adding a dependency).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|b| *b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|b| *b as u32).unwrap_or(0);
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(b2 & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
