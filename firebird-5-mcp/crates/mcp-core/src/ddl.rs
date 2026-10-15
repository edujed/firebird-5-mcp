//! DDL operations: triggers, procedures, functions, generators, drops.
//!
//! These use the driver's `prepare` + `execute` path, which sends the full
//! SQL text (including `SET TERM ^` blocks) to the server in a single
//! prepare call — so no isql shell-out is needed.

use rsfbclient::{Execute, FbError};

use crate::client::FirebirdClient;

/// Creates a new trigger. The `body` should contain only the SQL statements
/// (without BEGIN/END or SET TERM).
pub fn create_trigger(
    client: &mut FirebirdClient,
    trigger_name: &str,
    table_name: &str,
    trigger_type: &str,
    timing: &str,
    body: &str,
) -> Result<(), FbError> {
    let trigger_type = trigger_type.to_uppercase();
    let timing = timing.to_uppercase();

    if !matches!(trigger_type.as_str(), "INSERT" | "UPDATE" | "DELETE") {
        return Err(FbError::Other(format!(
            "invalid trigger type: {trigger_type}. Must be INSERT, UPDATE, or DELETE"
        )));
    }
    if !matches!(timing.as_str(), "BEFORE" | "AFTER") {
        return Err(FbError::Other(format!(
            "invalid timing: {timing}. Must be BEFORE or AFTER"
        )));
    }

    let sql = format!(
        "SET TERM ^ ;\nCREATE TRIGGER \"{trigger_name}\" FOR \"{table_name}\"\n{timing} {trigger_type}\nAS\nBEGIN\n{body}\nEND ^\nSET TERM ; ^"
    );

    exec_block(client, &sql)
}

/// Creates a new stored procedure. The `body` should contain only the PSQL
/// statements (without BEGIN/END or SET TERM).
pub fn create_procedure(
    client: &mut FirebirdClient,
    procedure_name: &str,
    params: &str,
    body: &str,
) -> Result<(), FbError> {
    let params = params.trim();
    let sql = if params.is_empty() {
        format!(
            "SET TERM ^ ;\nCREATE PROCEDURE {procedure_name}\nAS\nBEGIN\n{body}\nEND ^\nSET TERM ; ^"
        )
    } else {
        format!(
            "SET TERM ^ ;\nCREATE PROCEDURE {procedure_name}({params})\nAS\nBEGIN\n{body}\nEND ^\nSET TERM ; ^"
        )
    };
    exec_block(client, &sql)
}

/// Creates a new user-defined function. The `body` should contain only the
/// PSQL statements (without BEGIN/END or SET TERM).
pub fn create_function(
    client: &mut FirebirdClient,
    function_name: &str,
    params: &str,
    return_type: &str,
    body: &str,
) -> Result<(), FbError> {
    let params = params.trim();
    let sql = if params.is_empty() {
        format!(
            "SET TERM ^ ;\nCREATE FUNCTION {function_name} RETURNS {return_type}\nAS\nBEGIN\n{body}\nEND ^\nSET TERM ; ^"
        )
    } else {
        format!(
            "SET TERM ^ ;\nCREATE FUNCTION {function_name}({params}) RETURNS {return_type}\nAS\nBEGIN\n{body}\nEND ^\nSET TERM ; ^"
        )
    };
    exec_block(client, &sql)
}

/// Creates a new sequence (generator).
pub fn create_sequence(client: &mut FirebirdClient, sequence_name: &str) -> Result<(), FbError> {
    let sql = format!("CREATE SEQUENCE \"{sequence_name}\"");
    client.conn.execute(&sql, ())?;
    Ok(())
}

/// Drops a database object by type and name.
pub fn drop(client: &mut FirebirdClient, obj_type: &str, name: &str) -> Result<(), FbError> {
    let obj_type = obj_type.to_uppercase();
    let sql = match obj_type.as_str() {
        "TABLE" => format!("DROP TABLE \"{name}\""),
        "VIEW" => format!("DROP VIEW \"{name}\""),
        "DOMAIN" => format!("DROP DOMAIN \"{name}\""),
        "TRIGGER" => format!("DROP TRIGGER \"{name}\""),
        "PROCEDURE" => format!("DROP PROCEDURE \"{name}\""),
        "FUNCTION" => format!("DROP FUNCTION \"{name}\""),
        "INDEX" => format!("DROP INDEX \"{name}\""),
        "SEQUENCE" => format!("DROP SEQUENCE \"{name}\""),
        other => {
            return Err(FbError::Other(format!(
                "invalid type: {other}. Valid types: TABLE, VIEW, DOMAIN, TRIGGER, PROCEDURE, FUNCTION, INDEX, SEQUENCE"
            )));
        }
    };
    client.conn.execute(&sql, ())?;
    Ok(())
}

/// Changes the data type of a column to use a domain.
pub fn alter_column_type(
    client: &mut FirebirdClient,
    table: &str,
    column: &str,
    domain: &str,
) -> Result<(), FbError> {
    let sql = format!("ALTER TABLE {table} ALTER {column} TYPE {domain}");
    client.conn.execute(&sql, ())?;
    Ok(())
}

/// Executes a SQL block that may contain SET TERM delimiters.
///
/// Uses `SimpleConnection::execute` which handles the transaction
/// lifecycle automatically. The full SQL text (including `^` markers)
/// is sent to the server in a single execute call.
fn exec_block(client: &mut FirebirdClient, sql: &str) -> Result<(), FbError> {
    use rsfbclient::Execute;
    client.conn.execute(sql, ())?;
    Ok(())
}
