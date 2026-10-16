//! Batch operations: script execution, batch inserts, count, sample.

use rsfbclient::{Execute, FbError};
use rsfbclient_core::SqlType;

use crate::client::{FirebirdClient, IsolationPreset};

use super::split::split_statements;
use super::{QueryResult, Value, query};

/// Executes multiple SQL statements in a single transaction.
/// If any statement fails, the entire transaction is rolled back.
/// Returns the number of statements executed.
///
/// The script is split into individual statements using
/// [`split_statements`], which handles `SET TERM` directives (consumed,
/// not emitted), `BEGIN`/`END` nesting, strings, and comments. Each
/// resulting statement is sent to the driver as a complete unit, so
/// semicolons inside a PSQL body are not misinterpreted as separators.
///
/// `preset` controls the transaction isolation level (see
/// [`IsolationPreset`]). `lock_wait` is the maximum seconds to wait for a
/// lock (`None` = wait indefinitely). Use [`IsolationPreset::ReadOnly`]
/// to run audit/verification scripts that must not write.
pub fn run_script(
    client: &mut FirebirdClient,
    script: &str,
    preset: IsolationPreset,
    lock_wait: Option<u32>,
) -> Result<usize, FbError> {
    let statements = split_statements(script);
    if statements.is_empty() {
        return Err(FbError::Other("no statements found in script".into()));
    }

    client.begin_transaction(preset, lock_wait)?;
    let mut executed = 0;
    let result: Result<(), FbError> = (|| {
        for stmt in &statements {
            client.conn.execute(stmt, ())?;
            executed += 1;
        }
        Ok(())
    })();

    match result {
        Ok(()) => client.conn.commit()?,
        Err(e) => {
            let _ = client.conn.rollback();
            return Err(FbError::Other(format!("statement {executed} failed: {e}")));
        }
    }
    Ok(executed)
}

/// Inserts multiple rows into a table using a prepared statement within a
/// single transaction. Returns the number of rows inserted.
pub fn insert_batch(
    client: &mut FirebirdClient,
    table: &str,
    columns: &[String],
    rows: &[Vec<serde_json::Value>],
) -> Result<usize, FbError> {
    if columns.is_empty() {
        return Err(FbError::Other("no columns specified".into()));
    }
    if rows.is_empty() {
        return Err(FbError::Other("no rows to insert".into()));
    }

    let col_list = columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = vec!["?"; columns.len()].join(", ");
    let insert_sql = format!("INSERT INTO \"{table}\" ({col_list}) VALUES ({placeholders})");

    client.begin_rw_transaction()?;
    let mut inserted = 0;
    let result: Result<(), FbError> = (|| {
        for (i, row) in rows.iter().enumerate() {
            if row.len() != columns.len() {
                return Err(FbError::Other(format!(
                    "row {} has {} values, expected {}",
                    i + 1,
                    row.len(),
                    columns.len()
                )));
            }
            let params: Vec<SqlType> = row
                .iter()
                .map(json_value_to_sql)
                .collect::<Result<Vec<_>, _>>()?;
            client.conn.execute(&insert_sql, params)?;
            inserted += 1;
        }
        Ok(())
    })();

    match result {
        Ok(()) => client.conn.commit()?,
        Err(e) => {
            let _ = client.conn.rollback();
            return Err(FbError::Other(format!("row {inserted} failed: {e}")));
        }
    }
    Ok(inserted)
}

/// Converts a JSON value into a Firebird SQL parameter.
fn json_value_to_sql(v: &serde_json::Value) -> Result<SqlType, FbError> {
    Ok(match v {
        serde_json::Value::Null => SqlType::Null,
        serde_json::Value::Bool(b) => SqlType::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlType::Integer(i)
            } else {
                SqlType::Floating(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => SqlType::Text(s.clone()),
        other => {
            return Err(FbError::Other(format!(
                "unsupported JSON value for SQL parameter: {other}"
            )));
        }
    })
}

/// Returns the number of rows in a table, optionally filtered by WHERE.
pub fn count(client: &mut FirebirdClient, table: &str, where_clause: &str) -> Result<i64, FbError> {
    let mut sql = format!("SELECT COUNT(*) FROM \"{table}\"");
    if !where_clause.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(where_clause);
    }
    let result = query(client, &sql)?;
    if let Some(row) = result.rows.first() {
        for v in row {
            if let Value::Int(n) = v {
                return Ok(*n);
            }
        }
    }
    Ok(0)
}

/// Returns up to `limit` rows from a table with an optional WHERE clause.
pub fn sample(
    client: &mut FirebirdClient,
    table: &str,
    where_clause: &str,
    limit: usize,
) -> Result<QueryResult, FbError> {
    let limit = if limit == 0 { 10 } else { limit };
    let mut sql = format!("SELECT FIRST {limit} * FROM \"{table}\"");
    if !where_clause.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(where_clause);
    }
    query(client, &sql)
}
