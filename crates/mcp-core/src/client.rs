//! Thin wrapper around a Firebird connection (pure-Rust driver).
//!
//! Each [`FirebirdClient`] owns a single connection to one database file.
//! The MCP layer opens a client per tool call (matching the Go behavior), so
//! no connection pooling is needed at this level.

use rsfbclient::{FbError, SimpleConnection, builder_pure_rust};
use rsfbclient_core::{
    TrDataAccessMode, TrIsolationLevel, TrLockResolution, TrRecordVersion, TransactionConfiguration,
};

use crate::config::ServerConfig;

/// A connection to a single Firebird database.
pub struct FirebirdClient {
    pub(crate) conn: SimpleConnection,
    #[allow(dead_code)]
    pub(crate) server: ServerConfig,
    pub(crate) db_path: String,
}

/// Default transaction configuration for read-only metadata queries.
///
/// Uses SNAPSHOT (CONCURRENCY) isolation so that concurrent DDL (e.g. a
/// `CREATE TABLE` running in another session) does not cause "table or view
/// does not exist" errors while we read the system tables. The read-only
/// access mode prevents accidental writes.
fn metadata_transaction_conf() -> TransactionConfiguration {
    TransactionConfiguration {
        data_access: TrDataAccessMode::ReadOnly,
        isolation: TrIsolationLevel::Concurrency,
        lock_resolution: TrLockResolution::Wait(None),
    }
}

/// Transaction configuration for read-write operations (DML, DDL, scripts).
fn rw_transaction_conf() -> TransactionConfiguration {
    TransactionConfiguration {
        data_access: TrDataAccessMode::ReadWrite,
        isolation: TrIsolationLevel::ReadCommited(TrRecordVersion::NoRecordVersion),
        lock_resolution: TrLockResolution::Wait(None),
    }
}

/// Isolation presets for explicit transactions (scripts, batch writes).
///
/// Each preset maps to a validated combination of data access, isolation
/// level, and lock resolution. This avoids exposing the three independent
/// knobs (which can be combined inconsistently) and instead offers a small
/// set of sensible defaults.
///
/// Note: the `rsfbclient` driver (v0.29) only exposes three isolation
/// levels — `Concurrency` (snapshot), `Consistency` (table locking), and
/// `ReadCommited`. Firebird's `ReadStable` and `Serializable` levels are
/// not available through this driver, so they are not offered here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationPreset {
    /// Read-committed, no record version, wait for locks. The default for
    /// DML/DDL/scripts.
    #[default]
    ReadCommited,
    /// Snapshot (CONCURRENCY) isolation: the transaction sees a consistent
    /// snapshot of the database from the moment it started. Useful for
    /// scripts that read and rewrite data and need a stable view.
    Snapshot,
    /// Table locking (CONSISTENCY): the transaction locks all tables it
    /// touches, preventing concurrent writes. The strictest level available
    /// through the driver. Use for critical scripts where consistency
    /// matters more than throughput.
    Consistency,
    /// Read-only snapshot: the transaction cannot write. Useful for
    /// audit/verification scripts that must not accidentally modify data.
    ReadOnly,
}

impl IsolationPreset {
    /// Parses the string form accepted by the MCP `isolation` parameter.
    /// Returns `None` for unrecognized values.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "readcommitted" | "read_committed" | "read-committed" => Some(Self::ReadCommited),
            "snapshot" => Some(Self::Snapshot),
            "consistency" | "table_locking" | "table-locking" => Some(Self::Consistency),
            "readonly" | "read_only" | "read-only" => Some(Self::ReadOnly),
            _ => None,
        }
    }

    /// Human-readable label (for error messages and tool output).
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadCommited => "read-committed",
            Self::Snapshot => "snapshot",
            Self::Consistency => "consistency (table locking)",
            Self::ReadOnly => "read-only",
        }
    }

    /// Builds the `TransactionConfiguration` for this preset with the given
    /// lock-wait timeout (in seconds). `None` means wait indefinitely.
    fn to_conf(self, lock_wait: Option<u32>) -> TransactionConfiguration {
        let lock = match lock_wait {
            Some(secs) => TrLockResolution::Wait(Some(secs)),
            None => TrLockResolution::Wait(None),
        };
        match self {
            Self::ReadCommited => TransactionConfiguration {
                data_access: TrDataAccessMode::ReadWrite,
                isolation: TrIsolationLevel::ReadCommited(TrRecordVersion::NoRecordVersion),
                lock_resolution: lock,
            },
            Self::Snapshot => TransactionConfiguration {
                data_access: TrDataAccessMode::ReadWrite,
                isolation: TrIsolationLevel::Concurrency,
                lock_resolution: lock,
            },
            Self::Consistency => TransactionConfiguration {
                data_access: TrDataAccessMode::ReadWrite,
                isolation: TrIsolationLevel::Consistency,
                lock_resolution: lock,
            },
            Self::ReadOnly => TransactionConfiguration {
                data_access: TrDataAccessMode::ReadOnly,
                isolation: TrIsolationLevel::Concurrency,
                lock_resolution: lock,
            },
        }
    }
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
            .transaction(metadata_transaction_conf())
            .connect()?;
        let conn: SimpleConnection = conn_raw.into();
        Ok(Self {
            conn,
            server: server.clone(),
            db_path: db_path.to_string(),
        })
    }

    /// Creates a new Firebird database file at `path` with the given `page_size`
    /// and attaches to it. The path must be within the configured allowed paths.
    ///
    /// Valid page sizes are powers of two between 1024 and 32768 (1K–32K).
    /// The caller is responsible for validating the value; this method passes
    /// it straight through to the driver.
    pub fn create_database(
        server: &ServerConfig,
        path: &str,
        page_size: u32,
    ) -> Result<Self, FbError> {
        let conn_raw = builder_pure_rust()
            .host(&server.host)
            .port(server.port)
            .db_name(path)
            .user(&server.user)
            .pass(&server.password)
            .page_size(page_size)
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

    /// Begins an explicit read-write transaction on this connection.
    ///
    /// The connection is created with a read-only default transaction
    /// configuration (for metadata queries). This method starts a new
    /// transaction with read-write access, which is required for DML, DDL,
    /// and script execution.
    pub fn begin_rw_transaction(&mut self) -> Result<(), FbError> {
        self.conn.begin_transaction_config(rw_transaction_conf())?;
        Ok(())
    }

    /// Begins an explicit transaction with the given isolation preset and
    /// lock-wait timeout. Use this for scripts and batch operations where
    /// the caller wants to control the isolation level (e.g. `Snapshot` for
    /// a consistent read-rewrite, or `ReadOnly` to prevent accidental
    /// writes).
    ///
    /// `lock_wait` is the maximum number of seconds to wait for a lock
    /// before failing. `None` waits indefinitely.
    pub fn begin_transaction(
        &mut self,
        preset: IsolationPreset,
        lock_wait: Option<u32>,
    ) -> Result<(), FbError> {
        self.conn
            .begin_transaction_config(preset.to_conf(lock_wait))?;
        Ok(())
    }
}
