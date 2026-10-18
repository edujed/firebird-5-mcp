//! Firebird ↔ Arrow type mapping.
//!
//! - Export: Firebird column types → Arrow `DataType`
//! - Import: Arrow `DataType` → Firebird DDL type string
//! - Append: compatibility check between Arrow schema and Firebird table

use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use firebird_5_mcp_types::ColumnMeta;

/// Maps a Firebird column (from RDB$ introspection) to an Arrow `DataType`.
///
/// `type_num` is the RDB$FIELD_TYPE code, `precision` and `scale` come from
/// RDB$FIELD_PRECISION / RDB$FIELD_SCALE, `size` is RDB$FIELD_LENGTH (bytes),
/// `sub_type` is RDB$FIELD_SUB_TYPE (BLOB: 1=TEXT, 2=BINARY).
///
/// Note: the current export path uses [`column_meta_to_arrow`] instead,
/// because `ColumnMeta` (from query results) does not carry precision/scale.
/// This function is available for future RDB$-based export.
pub fn firebird_to_arrow(
    type_num: i64,
    precision: i64,
    scale: i64,
    _size: i64,
    _sub_type: i64,
) -> DataType {
    match type_num {
        // Integer types
        7 => DataType::Int16,  // SQL_SHORT (SMALLINT)
        8 => DataType::Int32,  // SQL_LONG (INTEGER)
        9 => DataType::Int64,  // SQL_INT64 (BIGINT)
        10 => DataType::Int64, // SQL_INT128 (INT128) — lossy, truncated to i64
        // Floating point
        279 => DataType::Float32, // SQL_FLOAT
        281 => DataType::Float64, // SQL_DOUBLE
        // Fixed-point (NUMERIC/DECIMAL)
        271 | 272 => {
            let p = if precision > 0 { precision as u8 } else { 18 };
            let s = if scale < 0 { (-scale) as i8 } else { 0 };
            DataType::Decimal128(p, s)
        }
        // Character types
        14 | 27 | 35 | 36 => DataType::Utf8, // CHAR, VARCHAR, NVARCHAR, RDB$ types
        // BLOB
        261 => DataType::Binary, // BLOB (both SUB_TYPE 1 and 2)
        // Temporal
        357 => DataType::Date32,                        // SQL_TYPE_DATE
        358 => DataType::Time64(TimeUnit::Microsecond), // SQL_TYPE_TIME
        359 => DataType::Timestamp(TimeUnit::Microsecond, None), // SQL_TIMESTAMP
        // Boolean
        29 => DataType::Boolean, // SQL_BOOLEAN
        // Fallback
        _ => DataType::Utf8,
    }
}

/// Maps an Arrow `DataType` to a Firebird DDL type string.
///
/// Used when creating a table from a file schema (`create` mode).
pub fn arrow_to_firebird(dt: &DataType) -> String {
    match dt {
        DataType::Int16 => "SMALLINT".to_string(),
        DataType::Int32 => "INTEGER".to_string(),
        DataType::Int64 => "BIGINT".to_string(),
        DataType::Float32 => "FLOAT".to_string(),
        DataType::Float64 => "DOUBLE PRECISION".to_string(),
        DataType::Decimal128(p, s) => format!("NUMERIC({p},{s})"),
        DataType::Utf8 => "VARCHAR(255)".to_string(),
        DataType::Boolean => "BOOLEAN".to_string(),
        DataType::Date32 => "DATE".to_string(),
        DataType::Time64(_) => "TIME".to_string(),
        DataType::Timestamp(_, _) => "TIMESTAMP".to_string(),
        DataType::Binary => "BLOB SUB_TYPE 1".to_string(),
        _ => "VARCHAR(255)".to_string(),
    }
}

/// Builds an Arrow `Schema` from `ColumnMeta` (type info from query results).
///
/// This is what the export path uses. NUMERIC/DECIMAL columns (detected via
/// `ColumnMeta.is_numeric`) are mapped to `Float64` — the vendor patch renders
/// them as exact decimal text (e.g. `"80.00"`), and Float64 preserves the value
/// through the Arrow → JSON → Firebird import round-trip.
pub fn column_meta_to_arrow(columns: &[ColumnMeta]) -> Schema {
    let fields: Vec<Field> = columns
        .iter()
        .map(|c| {
            // NUMERIC/DECIMAL columns (detected by the query layer) map to
            // Float64 — the vendor patch renders them as exact decimal text
            // (e.g. "80.00"), and Float64 preserves the value through the
            // Arrow → JSON → Firebird import round-trip.
            let dt = if c.is_numeric {
                DataType::Float64
            } else {
                match c.type_name.to_uppercase().as_str() {
                    "SMALLINT" => DataType::Int16,
                    "INTEGER" => DataType::Int32,
                    "BIGINT" => DataType::Int64,
                    "INT128" => DataType::Int64,
                    "FLOAT" => DataType::Float32,
                    "DOUBLE PRECISION" => DataType::Float64,
                    "BOOLEAN" => DataType::Boolean,
                    "DATE" => DataType::Date32,
                    "TIME" => DataType::Time64(TimeUnit::Microsecond),
                    "TIMESTAMP" => DataType::Timestamp(TimeUnit::Microsecond, None),
                    "BLOB" => DataType::Binary,
                    // VARCHAR, CHAR, NVARCHAR, and unknown → Utf8
                    _ => DataType::Utf8,
                }
            };
            Field::new(&c.name, dt, true)
        })
        .collect();
    Schema::new(fields)
}

/// Checks whether an Arrow schema is compatible with an existing Firebird
/// table for `append` mode.
///
/// Compatibility rules:
/// - Column count must match
/// - Column names must match (case-insensitive, trimmed)
///
/// Type compatibility is not strictly checked here — the actual `INSERT`
/// will fail with a clear error if the types are truly incompatible.
pub fn check_append_compatibility(
    arrow_schema: &Schema,
    firebird_columns: &[(String, i64)],
) -> Result<(), String> {
    if arrow_schema.fields().len() != firebird_columns.len() {
        return Err(format!(
            "column count mismatch: file has {}, table has {}",
            arrow_schema.fields().len(),
            firebird_columns.len()
        ));
    }

    for (i, field) in arrow_schema.fields().iter().enumerate() {
        let (fb_name, _fb_type_num) = &firebird_columns[i];
        if field.name().to_uppercase() != fb_name.trim().to_uppercase() {
            return Err(format!(
                "column {i}: name mismatch — file has '{}', table has '{}'",
                field.name(),
                fb_name
            ));
        }
    }

    Ok(())
}
