//! Query execution: result types, SELECT/DML/DDL execution.
//!
//! Sub-modules:
//! - [`split`] — SQL statement splitting (`SET TERM`, `BEGIN`/`END`)
//! - [`row_limit`] — `FIRST`/`SKIP` clause injection
//! - [`batch`] — script execution, batch inserts, count, sample

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rsfbclient::{Execute, FbError, Queryable};
use rsfbclient_core::SqlType;

use crate::client::FirebirdClient;

pub mod batch;
pub mod row_limit;
pub mod split;

pub use batch::{count, insert_batch, run_script, sample};
pub use row_limit::{DEFAULT_ROWS, MAX_ROWS, accepts_row_limit, inject_row_limit};
pub use split::split_statements;

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
                result.columns.push(ColumnMeta { name, type_name });
            }
        }
        result
            .rows
            .push((0..row.cols.len()).map(|i| row_value(&row, i)).collect());
    }

    result
}

/// Maps a raw Firebird SQL type code to a human-readable name.
fn sql_type_name(raw_type: u32) -> String {
    match raw_type {
        7 => "SMALLINT".to_string(),
        8 => "INTEGER".to_string(),
        9 => "DATE".to_string(),
        10 => "TIME".to_string(),
        11 => "FLOAT".to_string(),
        12 => "DATE".to_string(),
        14 => "CHAR".to_string(),
        16 => "BIGINT".to_string(),
        20 => "VARCHAR".to_string(),
        23 => "BOOLEAN".to_string(),
        27 => "DOUBLE".to_string(),
        29 => "TIMESTAMP".to_string(),
        35 => "TIMESTAMP".to_string(),
        37 => "VARCHAR".to_string(),
        40 => "DECIMAL".to_string(),
        43 => "NUMERIC".to_string(),
        45 => "BLOB".to_string(),
        48 => "NVARCHAR".to_string(),
        49 => "BOOLEAN".to_string(),
        261 => "BLOB".to_string(),
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
