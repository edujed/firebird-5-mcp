//! Firebird [`Value`] → primitive conversion helpers.
//!
//! Used by the export path to render values as text (CSV) or coerce them
//! into typed Arrow arrays.

use firebird_5_mcp_types::Value;

/// Renders a `Value` as a display string.
///
/// - `Null` → `""`
/// - `Binary` → `"<BLOB>"` (placeholder; binary is handled separately)
/// - temporal → ISO format (`YYYY-MM-DD`, `HH:MM:SS`, `YYYY-MM-DD HH:MM:SS`)
pub fn value_to_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Text(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        Value::Time(t) => t.format("%H:%M:%S").to_string(),
        Value::Timestamp(ts) => ts.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::Binary(_) => "<BLOB>".to_string(),
    }
}

/// Coerces a `Value` to `i64`. Non-numeric values yield `0`.
pub fn int_or_zero(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        Value::Text(s) => s.parse().unwrap_or(0),
        Value::Float(f) => *f as i64,
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

/// Coerces a `Value` to `f64`. Non-numeric values yield `0.0`.
pub fn float_or_zero(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Coerces a `Value` to `bool`. Non-boolean values yield `false`.
pub fn bool_or_false(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Text(s) => s.eq_ignore_ascii_case("true") || s == "1",
        _ => false,
    }
}
