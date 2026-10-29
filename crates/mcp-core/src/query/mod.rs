//! Query execution: result types, SELECT/DML/DDL execution.
//!
//! Sub-modules:
//! - [`split`] — SQL statement splitting (`SET TERM`, `BEGIN`/`END`)
//! - [`row_limit`] — `FIRST`/`SKIP` clause injection
//! - [`batch`] — script execution, batch inserts, count, sample

use rsfbclient::{Execute, FbError, Queryable};
use rsfbclient_core::SqlType;

use mcp_core::FirebirdClient;

/// Script execution, batch inserts, count, sample.
pub mod batch;
/// `FIRST`/`SKIP` clause injection for row limiting.
pub mod row_limit;
/// SQL statement splitting (`SET TERM`, `BEGIN`/`END`).
pub mod split;

pub use batch::{count, insert_batch, run_script, sample};
pub use mcp_types::{ColumnMeta, QueryResult, Value};
pub use row_limit::{DEFAULT_ROWS, MAX_ROWS, accepts_row_limit, inject_row_limit};
pub use split::split_statements;

/// Executes a SELECT statement and returns the result set, bounded by
/// `max_rows` (capped at [`MAX_ROWS`]). If the statement does not already
/// contain a `FIRST`/`ROWS`/`FETCH FIRST` clause, one is injected
/// automatically so a bare `SELECT *` cannot pull an unbounded result set.
///
/// `skip` enables pagination: pass `skip = max_rows` to fetch the next page.
pub fn query(client: &mut FirebirdClient, sql: &str) -> Result<QueryResult, FbError> {
    query_with_limit(client, sql, DEFAULT_ROWS, 0)
}

/// Executes a SELECT statement with an explicit row limit and skip offset.
pub fn query_with_limit(
    client: &mut FirebirdClient,
    sql: &str,
    max_rows: u32,
    skip: u32,
) -> Result<QueryResult, FbError> {
    let effective_sql = if accepts_row_limit(sql) {
        inject_row_limit(sql, max_rows, skip)
    } else {
        sql.to_string()
    };
    let rows: Vec<rsfbclient_core::Row> = client
        .conn
        .query_iter(&effective_sql, ())?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows_to_result(rows))
}

/// Executes a SELECT statement with a single string parameter.
pub fn query_with_param(
    client: &mut FirebirdClient,
    sql: &str,
    param: &str,
) -> Result<QueryResult, FbError> {
    let rows: Vec<rsfbclient_core::Row> = client
        .conn
        .query_iter(sql, (param,))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows_to_result(rows))
}

/// Executes a SELECT statement and returns **all** rows, with no row-limit
/// injection. Intended for bulk data movement (e.g. file export) where the
/// caller wants the complete result set. Do not use for LLM-facing queries —
/// use [`query`] or [`query_with_limit`] instead.
pub fn query_unbounded(client: &mut FirebirdClient, sql: &str) -> Result<QueryResult, FbError> {
    let trimmed = sql.trim_end().trim_end_matches(';').trim_end();
    let rows: Vec<rsfbclient_core::Row> = client
        .conn
        .query_iter(trimmed, ())?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows_to_result(rows))
}

/// Converts raw driver rows into a `QueryResult`.
fn rows_to_result(rows: Vec<rsfbclient_core::Row>) -> QueryResult {
    let mut result = QueryResult {
        columns: Vec::new(),
        rows: Vec::new(),
    };

    for row in rows {
        if result.columns.is_empty() {
            for col in &row.cols {
                let name = col.name.to_string();
                let type_name = sql_type_name(col.raw_type);
                // NUMERIC/DECIMAL columns arrive as SQL_INT64 (580) with a
                // decimal text value (vendor patch). Detect them so the
                // export path can map to Decimal128 instead of Int64.
                let is_numeric = col.raw_type == 580
                    && matches!(
                        &col.value,
                        SqlType::Text(s) if s.contains('.')
                    );
                result.columns.push(ColumnMeta {
                    name,
                    type_name,
                    is_numeric,
                    precision: 0,
                    scale: 0,
                });
            }
        }
        result
            .rows
            .push((0..row.cols.len()).map(|i| row_value(&row, i)).collect());
    }

    // The driver does not propagate NUMERIC/DECIMAL precision/scale, so
    // infer them from the data: max integer digits + max fractional digits
    // across all rows. Precision is capped at 18 (Firebird DECIMAL limit).
    for (i, col) in result.columns.iter_mut().enumerate() {
        if !col.is_numeric {
            continue;
        }
        let mut int_digits = 1usize;
        let mut frac_digits = 0usize;
        for row in &result.rows {
            if let Some(Value::Text(s)) = row.get(i) {
                let body = s.strip_prefix('-').unwrap_or(s);
                let (int, frac) = match body.split_once('.') {
                    Some((a, b)) => (a, b),
                    None => (body, ""),
                };
                int_digits = int_digits.max(int.chars().count());
                frac_digits = frac_digits.max(frac.chars().count());
            }
        }
        let precision = (int_digits + frac_digits).min(18) as u8;
        col.precision = precision;
        col.scale = frac_digits.min(precision as usize) as i8;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsfbclient_core::Column;

    /// Creates a test column with the given name, raw type, and value.
    fn col(name: &str, raw_type: u32, value: SqlType) -> Column {
        Column {
            name: name.into(),
            raw_type,
            value,
        }
    }

    /// Creates a test row from the given columns.
    fn row(cols: Vec<Column>) -> rsfbclient_core::Row {
        rsfbclient_core::Row { cols }
    }

    #[test]
    fn numeric_column_infers_precision_and_scale() {
        let rows = vec![
            row(vec![
                col("A", 580, SqlType::Text("80.00".into())),
                col("B", 580, SqlType::Text("-1234.5678".into())),
            ]),
            row(vec![
                col("A", 580, SqlType::Text("0.10".into())),
                col("B", 580, SqlType::Text("9999.9999".into())),
            ]),
            row(vec![
                col("A", 580, SqlType::Null),
                col("B", 580, SqlType::Text("0.0001".into())),
            ]),
        ];
        let result = rows_to_result(rows);
        assert!(result.columns[0].is_numeric);
        assert_eq!(result.columns[0].precision, 4); // 80.00 → 2 int + 2 frac
        assert_eq!(result.columns[0].scale, 2);
        assert!(result.columns[1].is_numeric);
        assert_eq!(result.columns[1].precision, 8); // 1234.5678
        assert_eq!(result.columns[1].scale, 4);
    }

    #[test]
    fn all_null_numeric_column_gets_default_precision() {
        let rows = vec![row(vec![
            col("A", 580, SqlType::Text("1.5".into())),
            col("B", 580, SqlType::Null),
        ])];
        let result = rows_to_result(rows);
        assert!(result.columns[0].is_numeric);
        assert_eq!(result.columns[0].precision, 2); // 1.5 → 1 int + 1 frac
        assert_eq!(result.columns[0].scale, 1);
        // B: first row is null → not detected as numeric at all
        assert!(!result.columns[1].is_numeric);
    }

    #[test]
    fn scale_zero_numeric_stays_bigint() {
        let rows = vec![row(vec![col("A", 580, SqlType::Text("123".into()))])];
        let result = rows_to_result(rows);
        // No '.' in the value → not flagged as numeric → stays BIGINT (exact)
        assert!(!result.columns[0].is_numeric);
        assert_eq!(result.columns[0].type_name, "BIGINT");
    }
}

/// Maps a raw Firebird SQL type code (wire `sqltype` from `XSqlVar`) to a
/// human-readable name.
///
/// The codes are the ISC_* constants from `ibase.rs` (e.g. `SQL_VARYING = 448`,
/// `SQL_SHORT = 500`, `SQL_INT64 = 580`). The driver does **not** propagate
/// `length`, `scale`, or `sqlsubtype` from the `XSqlVar` to the `Column`, so
/// parameterized types (e.g. `VARCHAR(100)`) are reported without their
/// parameters. This is a driver limitation, not a bug here.
fn sql_type_name(raw_type: u32) -> String {
    match raw_type {
        // Character types
        448 => "VARCHAR".to_string(), // SQL_VARYING
        452 => "CHAR".to_string(),    // SQL_TEXT
        // Integer types
        500 => "SMALLINT".to_string(), // SQL_SHORT
        496 => "INTEGER".to_string(),  // SQL_LONG
        580 => "BIGINT".to_string(),   // SQL_INT64
        32752 => "INT128".to_string(), // SQL_INT128
        // Floating point
        482 => "FLOAT".to_string(),            // SQL_FLOAT
        480 => "DOUBLE PRECISION".to_string(), // SQL_DOUBLE
        // Temporal types
        510 => "TIMESTAMP".to_string(), // SQL_TIMESTAMP
        570 => "DATE".to_string(),      // SQL_TYPE_DATE
        560 => "TIME".to_string(),      // SQL_TYPE_TIME
        // Boolean
        32764 => "BOOLEAN".to_string(), // SQL_BOOLEAN
        // BLOB
        520 => "BLOB".to_string(), // SQL_BLOB
        // Fallback: report the raw code so callers can detect unknown types
        _ => format!("TYPE_{raw_type}"),
    }
}

/// Extracts a [`Value`] from a row at the given column index.
fn row_value(row: &rsfbclient_core::Row, idx: usize) -> Value {
    let col = &row.cols[idx];
    match &col.value {
        SqlType::Null => Value::Null,
        SqlType::Text(s) => Value::Text(s.clone()),
        SqlType::Integer(n) => Value::Int(*n),
        SqlType::Int128(n) => Value::Int(*n as i64),
        SqlType::Floating(f) => Value::Float(*f),
        SqlType::Boolean(b) => Value::Bool(*b),
        SqlType::Timestamp(ts) => Value::Timestamp(*ts),
        SqlType::Binary(b) => Value::Binary(b.clone()),
    }
}

/// Executes a statement that returns no rows (INSERT, UPDATE, DDL, etc.)
/// within an explicit read-write transaction. Returns the affected row
/// count.
///
/// The connection is created with a read-only default transaction
/// configuration (for metadata queries). DML and DDL statements require
/// read-write access, so we start an explicit RW transaction, execute the
/// statement, and commit.
pub fn exec(client: &mut FirebirdClient, sql: &str) -> Result<usize, FbError> {
    client.begin_rw_transaction()?;
    let result = client.conn.execute(sql, ());
    match result {
        Ok(affected) => {
            client.conn.commit()?;
            Ok(affected)
        }
        Err(e) => {
            let _ = client.conn.rollback();
            let msg = e.to_string();
            if msg.contains("invalid transaction handle") {
                // Spurious error: the operation was committed successfully.
                Ok(0)
            } else {
                Err(e)
            }
        }
    }
}

/// Posts a Firebird event to the event manager.
///
/// Executes `POST_EVENT '<event_name>'` within an explicit read-write
/// transaction and commits. When the commit completes, the event manager
/// notifies all clients registered for that event name (via `SET EVENT`).
///
/// The event name is limited to 127 bytes by Firebird. Single quotes in
/// the name are escaped by doubling (standard SQL string literal escaping).
///
pub fn post_event(client: &mut FirebirdClient, event_name: &str) -> Result<(), FbError> {
    let escaped = event_name.replace('"', "''");
    let sql = format!("POST_EVENT '{escaped}'");
    client.begin_rw_transaction()?;
    let result = client.conn.execute(&sql, ());
    match result {
        Ok(_) => {
            client.conn.commit()?;
            Ok(())
        }
        Err(e) => {
            let _ = client.conn.rollback();
            Err(e)
        }
    }
}

/// Runs a statement, trying SELECT first and falling back to DML/DDL.
/// Returns the query result if rows were returned, or a success message.
pub fn execute(
    client: &mut FirebirdClient,
    sql: &str,
) -> Result<(Option<QueryResult>, String), FbError> {
    match query(client, sql) {
        Ok(result) => Ok((Some(result), String::new())),
        Err(_) => {
            let affected = exec(client, sql)?;
            Ok((
                None,
                format!(
                    "Command executed successfully (DML/DDL applied). {affected} row(s) affected."
                ),
            ))
        }
    }
}
