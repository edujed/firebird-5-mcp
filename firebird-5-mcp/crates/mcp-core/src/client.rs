//! Thin wrapper around a Firebird connection (pure-Rust driver).
//!
//! Each [`FirebirdClient`] owns a single connection to one database file.
//! The MCP layer opens a client per tool call (matching the Go behavior), so
//! no connection pooling is needed at this level.

use rsfbclient::{FbError, SimpleConnection, builder_pure_rust};

use crate::config::ServerConfig;

/// A connection to a single Firebird database.
pub struct FirebirdClient {
    pub(crate) conn: SimpleConnection,
    #[allow(dead_code)]
    pub(crate) server: ServerConfig,
    pub(crate) db_path: String,
}

impl FirebirdClient {
    /// Opens a connection to the database at `db_path` using the server config.
    pub fn new(server: &ServerConfig, db_path: &str) -> Result<Self, FbError> {
        let conn_raw = builder_pure_rust()
            .host(&server.host)
            .port(server.port)
            .db_name(db_path)
            .user(&server.user)
            .pass(&server.password)
            .connect()?;
        let conn: SimpleConnection = conn_raw.into();
        Ok(Self {
            conn,
            server: server.clone(),
            db_path: db_path.to_string(),
        })
    }

    /// Creates a new Firebird database file at `path` (page size 4096) and
    /// attaches to it. The path must be within the configured allowed paths.
    pub fn create_database(server: &ServerConfig, path: &str) -> Result<Self, FbError> {
        let conn_raw = builder_pure_rust()
            .host(&server.host)
            .port(server.port)
            .db_name(path)
            .user(&server.user)
            .pass(&server.password)
            .page_size(4096)
            .create_database()?;
        let conn: SimpleConnection = conn_raw.into();
        Ok(Self {
            conn,
            server: server.clone(),
            db_path: path.to_string(),
        })
    }

    /// The path of the database this client is attached to.
    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// Opens a second, independent connection to the same database.
    /// Used by [`crate::explain::explain_plan`] to read
    /// `MON$COMPILED_STATEMENTS` (per-connection) without clobbering the
    /// plan of the statement being explained.
    pub fn second_connection(&self) -> Result<SimpleConnection, FbError> {
        let conn_raw = builder_pure_rust()
            .host(&self.server.host)
            .port(self.server.port)
            .db_name(&self.db_path)
            .user(&self.server.user)
            .pass(&self.server.password)
            .connect()?;
        Ok(conn_raw.into())
    }

    /// Executes a SQL statement that may contain SET TERM blocks (e.g.
    /// `CREATE PROCEDURE`, `CREATE GENERATOR`, `CREATE FUNCTION`) within an
    /// auto-commit transaction. The full SQL text (including `^` markers) is
    /// sent to the server in a single prepare call, so no isql shell-out is
    /// needed. For regular DML/DDL, prefer [`crate::query::exec`].
    pub fn execute_immediate(&mut self, sql: &str) -> Result<(), FbError> {
        use rsfbclient::Execute;
        self.conn.execute(sql, ())?;
        Ok(())
    }
}
