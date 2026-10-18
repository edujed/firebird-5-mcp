//! Arrow array → `serde_json::Value` conversion.
//!
//! Used by the import path (Parquet/IPC) to convert Arrow values into
//! JSON values suitable for [`insert_batch`].

use arrow::array::{
    Array, ArrayRef, BinaryArray, BooleanArray, Date32Array, Decimal128Array, Float32Array,
    Float64Array, Int16Array, Int32Array, Int64Array, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::DataType;
use chrono::NaiveDate;

/// Converts a single Arrow array value to a `serde_json::Value`.
///
/// - `Binary` → hex string (Firebird interprets hex strings as BLOB data)
/// - `Date32` → `"YYYY-MM-DD"`
/// - `Time64` → `"HH:MM:SS.ffffff"`
/// - `Timestamp` → `"YYYY-MM-DD HH:MM:SS"`
/// - `Decimal128` → formatted decimal string
pub fn arrow_value_to_json(
    array: &ArrayRef,
    dt: &DataType,
    idx: usize,
) -> Result<serde_json::Value, String> {
    if array.is_null(idx) {
        return Ok(serde_json::Value::Null);
    }

    Ok(match dt {
        DataType::Int16 => {
            let a = downcast::<Int16Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Int32 => {
            let a = downcast::<Int32Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Int64 => {
            let a = downcast::<Int64Array>(array)?;
            serde_json::Value::Number(a.value(idx).into())
        }
        DataType::Float32 => {
            let a = downcast::<Float32Array>(array)?;
            serde_json::Value::Number(
                serde_json::Number::from_f64(a.value(idx) as f64).ok_or("invalid float")?,
            )
        }
        DataType::Float64 => {
            let a = downcast::<Float64Array>(array)?;
            serde_json::Value::Number(
                serde_json::Number::from_f64(a.value(idx)).ok_or("invalid float")?,
            )
        }
        DataType::Utf8 => {
            let a = downcast::<StringArray>(array)?;
            serde_json::Value::String(a.value(idx).to_string())
        }
        DataType::Boolean => {
            let a = downcast::<BooleanArray>(array)?;
            serde_json::Value::Bool(a.value(idx))
        }
        DataType::Binary => {
            let a = downcast::<BinaryArray>(array)?;
            serde_json::Value::String(a.value(idx).iter().map(|b| format!("{b:02x}")).collect())
        }
        DataType::Date32 => {
            let a = downcast::<Date32Array>(array)?;
            let days = a.value(idx);
            let date = NaiveDate::from_num_days_from_ce_opt(days)
                .ok_or_else(|| format!("invalid date: {days} days from CE"))?;
            serde_json::Value::String(date.format("%Y-%m-%d").to_string())
        }
        DataType::Time64(_) => {
            let a = downcast::<Time64MicrosecondArray>(array)?;
            let micros = a.value(idx);
            let secs = micros / 1_000_000;
            let frac = micros % 1_000_000;
            serde_json::Value::String(format!(
                "{:02}:{:02}:{:02}.{:06}",
                secs / 3600,
                (secs / 60) % 60,
                secs % 60,
                frac
            ))
        }
        DataType::Timestamp(_, _) => {
            let a = downcast::<TimestampMicrosecondArray>(array)?;
            let micros = a.value(idx);
            let dt = chrono::DateTime::from_timestamp_micros(micros)
                .ok_or_else(|| format!("invalid timestamp: {micros}"))?;
            serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        DataType::Decimal128(_, s) => {
            let a = downcast::<Decimal128Array>(array)?;
            let unscaled = a.value(idx);
            let scale = *s as i32;
            serde_json::Value::String(format_decimal(unscaled, scale))
        }
        other => {
            return Err(format!("unsupported Arrow type: {other:?}"));
        }
    })
}

/// Formats a `Decimal128` unscaled value with the given scale.
pub fn format_decimal(unscaled: i128, scale: i32) -> String {
    if scale == 0 {
        return unscaled.to_string();
    }
    let negative = unscaled < 0;
    let digits = unscaled.unsigned_abs().to_string();
    let body = if scale > 0 {
        let padded = format!("{:0>width$}", digits, width = scale as usize + 1);
        let split = padded.len() - scale as usize;
        format!("{}.{}", &padded[..split], &padded[split..])
    } else {
        let frac_len = (-scale) as usize;
        let padded = format!("{:0>width$}", digits, width = frac_len + 1);
        let split = padded.len() - frac_len;
        format!("{}.{}", &padded[..split], &padded[split..])
    };
    if negative { format!("-{body}") } else { body }
}

/// Downcasts an `ArrayRef` to a concrete Arrow array type.
fn downcast<T: Array + 'static>(array: &ArrayRef) -> Result<&T, String> {
    array
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| format!("cannot downcast to {:?}", std::any::TypeId::of::<T>()))
}
