//! Firebird type formatting and identifier validation.
//!
//! Ported from the Go implementation (`internal/firebird/client.go`).

use chrono::Timelike;

use crate::query::Value;

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
            let char_size = bytes_to_chars(size, &type_name);
            if char_size > 0 {
                return format!("{type_name}({char_size})");
            }
        }
        "BLOB" => {
            // For BLOB types (type_num 261), sub_type indicates the blob sub-type:
            // 1 = BINARY, 2 = TEXT, 3 = BLOB (generic)
            if sub_type == 2 {
                return "BLOB SUB_TYPE TEXT".to_string();
            } else if sub_type == 1 {
                return "BLOB SUB_TYPE BINARY".to_string();
            }
            return "BLOB".to_string();
        }
        _ => {}
    }
    type_name
}

/// Converts a byte size to character count for a given character type.
fn bytes_to_chars(byte_size: i64, type_name: &str) -> i64 {
    if byte_size <= 0 {
        return 0;
    }
    match type_name {
        "NVARCHAR" => byte_size / 2,
        _ => byte_size / 4, // CHAR, VARCHAR: assume UTF8 (4 bytes/char)
    }
}

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
