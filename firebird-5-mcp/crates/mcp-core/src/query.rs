//! Query execution: SELECT results, DML/DDL, scripts, batch inserts.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rsfbclient::{Execute, FbError, Queryable};
use rsfbclient_core::SqlType;

use crate::client::FirebirdClient;

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

/// Executes a SELECT statement and returns the full result set.
pub fn query(client: &mut FirebirdClient, sql: &str) -> Result<QueryResult, FbError> {
    let rows: Vec<rsfbclient_core::Row> = client
        .conn
        .query_iter(sql, ())?
        .collect::<Result<Vec<_>, _>>()?;

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

    Ok(result)
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

    Ok(result)
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
/// within an auto-commit transaction. Returns the affected row count.
///
/// Uses `SimpleConnection::execute` which handles the transaction
/// lifecycle automatically. Note: the driver may return a spurious
/// "invalid transaction handle" error even when the operation succeeds
/// (the commit happens before the error is raised). We treat this specific
/// error as a success since the operation is already committed.
pub fn exec(client: &mut FirebirdClient, sql: &str) -> Result<usize, FbError> {
    use rsfbclient::Execute;
    match client.conn.execute(sql, ()) {
        Ok(affected) => Ok(affected),
        Err(e) => {
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

/// Splits a SQL script into individual statements, handling semicolons
/// outside of strings and comments. Ported from the Go implementation.
pub fn split_statements(script: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    let chars: Vec<char> = script.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];

        if in_block_comment {
            if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                current.push(ch);
            }
            i += 1;
            continue;
        }
        if in_single_quote {
            if ch == '\'' && i + 1 < chars.len() && chars[i + 1] == '\'' {
                current.push('\'');
                current.push('\'');
                i += 2;
                continue;
            }
            if ch == '\'' {
                in_single_quote = false;
            }
            current.push(ch);
            i += 1;
            continue;
        }
        if in_double_quote {
            if ch == '"' {
                in_double_quote = false;
            }
            current.push(ch);
            i += 1;
            continue;
        }

        // Check for comment starts
        if ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            in_block_comment = true;
            i += 2;
            continue;
        }

        // Check for quote starts
        if ch == '\'' {
            in_single_quote = true;
            current.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' {
            in_double_quote = true;
            current.push(ch);
            i += 1;
            continue;
        }

        // Check for statement end
        if ch == ';' {
            let stmt = current.trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current.clear();
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    let stmt = current.trim().to_string();
    if !stmt.is_empty() {
        statements.push(stmt);
    }

    statements
}

/// Executes multiple SQL statements in a single transaction.
/// If any statement fails, the entire transaction is rolled back.
/// Returns the number of statements executed.
pub fn run_script(client: &mut FirebirdClient, script: &str) -> Result<usize, FbError> {
    let statements = split_statements(script);
    if statements.is_empty() {
        return Err(FbError::Other("no statements found in script".into()));
    }

    client.conn.begin_transaction()?;
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

    client.conn.begin_transaction()?;
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
            let params: Vec<rsfbclient_core::SqlType> = row
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
fn json_value_to_sql(v: &serde_json::Value) -> Result<rsfbclient_core::SqlType, FbError> {
    use rsfbclient_core::SqlType;
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
