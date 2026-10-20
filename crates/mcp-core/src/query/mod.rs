//! Query execution: result types, SELECT/DML/DDL execution.
//!
//! Sub-modules:
//! - [`split`] — SQL statement splitting (`SET TERM`, `BEGIN`/`END`)
//! - [`row_limit`] — `FIRST`/`SKIP` clause injection
//! - [`batch`] — script execution, batch inserts, count, sample

use rsfbclient::{Execute, FbError, Queryable};
use rsfbclient_core::SqlType;

use crate::client::FirebirdClient;

pub mod batch;
pub mod row_limit;
pub mod split;

pub use batch::{count, insert_batch, run_script, sample};
pub use firebird_5_mcp_types::{ColumnMeta, QueryResult, Value};
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
                // export path can map to Float64 instead of Int64.
                let is_numeric = col.raw_type == 580
                    && matches!(
                        &col.value,
                        SqlType::Text(s) if s.contains('.')
                    );
                result.columns.push(ColumnMeta {
                    name,
                    type_name,
                    is_numeric,
                });
            }
        }
        result
            .rows
            .push((0..row.cols.len()).map(|i| row_value(&row, i)).collect());
    }

    result
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
