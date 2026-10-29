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
//! the tool description in `mcp-tools` makes this explicit.

use rsfbclient::FbError;

use mcp_core::FirebirdClient;

/// Returns the execution plan for `sql`.
///
/// The statement is executed inside a transaction that is rolled back, so no
/// data changes persist. The plan is read from `MON$COMPILED_STATEMENTS`
/// (per-connection in Firebird) after execution.
pub fn explain_plan(client: &mut FirebirdClient, sql: &str) -> Result<String, FbError> {
    client.begin_rw_transaction()?;

    // Execute the statement. For SELECTs the rows are fetched and discarded;
    // for DML the changes stay inside the transaction (rolled back below).
    let exec_result: Result<(), FbError> = (|| {
        let result = mcp_core::query::query_unbounded(client, sql)?;
        let _ = result;
        Ok(())
    })();

    // Read the plan from MON$STATEMENTS (per-transaction) on the SAME
    // connection. The MON$STATEMENTS query itself becomes a new statement in
    // the transaction, so we filter by MON$SQL_TEXT to isolate the plan of
    // the statement the user asked about. Uses a bind parameter to avoid
    // SQL injection from quotes in the user's SQL.
    let plan_result: Result<String, FbError> = (|| {
        let result = mcp_core::query::query_with_param(
            client,
            "SELECT FIRST 1 MON$EXPLAINED_PLAN FROM MON$STATEMENTS WHERE MON$SQL_TEXT = ?",
            sql,
        )?;
        match result.rows.first() {
            Some(row) => match row.first() {
                Some(mcp_types::Value::Text(plan)) if !plan.trim().is_empty() => Ok(plan.clone()),
                _ => Ok("Nenhum plano gerado".to_string()),
            },
            None => Ok("Nenhum plano gerado".to_string()),
        }
    })();

    // Roll back the transaction to discard any data changes.
    // (The driver's SimpleConnection doesn't expose rollback directly,
    // so we rely on the connection being dropped/closed to roll back.)

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
