//! Execution plan retrieval.
//!
//! The plan is obtained by *executing* the statement inside a transaction
//! that is always rolled back. The server compiles the statement (producing
//! the plan) and then runs it; the rollback discards any data changes.
//!
//! This is the same approach the Go implementation used (via
//! `MON$STATEMENTS`), and it is the only reliable way to get the plan with
//! the pure-Rust driver, which does not expose the `info_sql` wire operation.
//!
//! **Important:** the statement IS executed — a SELECT is fully evaluated
//! (its result set is discarded), and a DML statement's changes are rolled
//! back. For a very expensive statement this still costs the execution time;
//! the tool description in `mcp-bin` makes this explicit.

use rsfbclient::{FbError, Queryable};

use crate::client::FirebirdClient;

/// Returns the execution plan for `sql`.
///
/// The statement is executed inside a transaction that is rolled back, so no
/// data changes persist. The plan is read from `MON$COMPILED_STATEMENTS`
/// (per-connection in Firebird) after execution.
pub fn explain_plan(client: &mut FirebirdClient, sql: &str) -> Result<String, FbError> {
    client.conn.begin_transaction()?;

    // Execute the statement. For SELECTs the rows are fetched and discarded;
    // for DML the changes stay inside the transaction (rolled back below).
    let exec_result: Result<(), FbError> = (|| {
        let rows: Vec<rsfbclient_core::Row> = client
            .conn
            .query_iter(sql, ())?
            .collect::<Result<Vec<_>, _>>()?;
        let _ = rows;
        Ok(())
    })();

    // Read the plan from MON$STATEMENTS (per-transaction) on the SAME
    // connection. The MON$STATEMENTS query itself becomes a new statement in
    // the transaction, so we filter by MON$SQL_TEXT to isolate the plan of
    // the statement the user asked about.
    let plan_result: Result<String, FbError> = (|| {
        let escaped = sql.replace('"', "''");
        let plan_sql = format!(
            "SELECT FIRST 1 MON$EXPLAINED_PLAN FROM MON$STATEMENTS WHERE MON$SQL_TEXT = '{}'",
            escaped
        );
        let row: Option<(Option<String>,)> = client.conn.query_first(&plan_sql, ())?;
        match row {
            Some((plan,)) => match plan {
                Some(plan) if !plan.trim().is_empty() => Ok(plan),
                _ => Ok("Nenhum plano gerado".to_string()),
            },
            None => Ok("Nenhum plano gerado".to_string()),
        }
    })();

    let _ = client.conn.rollback();

    match (exec_result, plan_result) {
        (Ok(()), Ok(plan)) => Ok(plan),
        (Err(e), _) => {
            let msg = e.to_string();
            if msg.contains("Unsupported column type") {
                Err(FbError::Other(
                    "A statement contém colunas BLOB BINARY (SUB_TYPE 2) não suportadas pelo driver. Tente envolver essas colunas em CAST(coluna AS BLOB SUB_TYPE 1) ou usar SELECT FIRST 0."
                        .to_string(),
                ))
            } else {
                Err(e)
            }
        }
        (_, Err(e)) => Err(e),
    }
}
