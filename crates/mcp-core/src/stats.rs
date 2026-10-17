//! Database statistics and encryption state (MON$ tables).
//!
//! Provides a snapshot of `MON$DATABASE` (plus `MON$IO_STATS` aggregates),
//! live attachments from `MON$ATTACHMENTS`, and live statements from
//! `MON$STATEMENTS`. Also reads the at-rest encryption state via
//! `MON$DATABASE.MON$CRYPT_STATE` (Firebird 3.0+).

use crate::client::FirebirdClient;
use crate::query::{self, Value};
use rsfbclient::FbError;

/// Encryption state of an attached Firebird database.
///
/// Mirrors the values reported by `MON$DATABASE.MON$CRYPT_STATE`
/// (Firebird 3.0+):
///
/// | Value | Meaning |
/// |-------|---------|
/// | `0`   | unencrypted |
/// | `1`   | encrypted |
/// | `2`   | decrypt in progress |
/// | `3`   | encrypt in progress |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptState {
    /// Database file is unencrypted.
    Unencrypted,
    /// Database file is encrypted.
    Encrypted,
    /// `ALTER DATABASE DECRYPT` is in progress.
    DecryptInProgress,
    /// `ALTER DATABASE ENCRYPT` is in progress.
    EncryptInProgress,
}

impl CryptState {
    /// Parses the raw integer reported by `MON$CRYPT_STATE`.
    pub fn from_raw(value: i64) -> Self {
        match value {
            1 => Self::Encrypted,
            2 => Self::DecryptInProgress,
            3 => Self::EncryptInProgress,
            _ => Self::Unencrypted,
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unencrypted => "unencrypted",
            Self::Encrypted => "encrypted",
            Self::DecryptInProgress => "decrypt in progress",
            Self::EncryptInProgress => "encrypt in progress",
        }
    }

    /// Returns `true` when the database is in the `Encrypted` state.
    pub const fn is_encrypted(self) -> bool {
        matches!(self, Self::Encrypted)
    }
}

/// Reads the at-rest encryption state of the attached database.
///
/// On Firebird 2.5 (no native encryption) the column does not exist; in
/// that case the factual answer is `Unencrypted`.
pub fn crypt_state(client: &mut FirebirdClient) -> Result<CryptState, FbError> {
    let result = query::query(client, "SELECT MON$CRYPT_STATE FROM MON$DATABASE")?;
    let value = result.get(0, 0).as_i64().unwrap_or(0);
    Ok(CryptState::from_raw(value))
}

/// Returns the engine version string (e.g. "5.0.0.1234").
pub fn engine_version(client: &mut FirebirdClient) -> Result<String, FbError> {
    let result = query::query(
        client,
        "SELECT rdb$get_context('SYSTEM', 'ENGINE_VERSION') FROM rdb$database",
    )?;
    Ok(result
        .get(0, 0)
        .as_text()
        .unwrap_or_else(|| "unknown".to_string()))
}

/// Returns the major engine version (e.g. `5` for Firebird 5).
pub fn engine_major(client: &mut FirebirdClient) -> Result<u32, FbError> {
    let version = engine_version(client)?;
    Ok(version
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
        .unwrap_or(0))
}

/// Snapshot of `MON$DATABASE` plus `MON$IO_STATS` aggregates.
pub struct MonDatabase {
    pub name: String,
    pub page_size: i64,
    pub pages: i64,
    pub oldest_transaction: i64,
    pub oldest_active: i64,
    pub oldest_snapshot: i64,
    pub next_transaction: i64,
    pub sweep_interval: i64,
    pub forced_writes: bool,
    pub read_only: bool,
    pub sql_dialect: i64,
    pub creation_date: String,
    pub backup_state: i64,
    pub shutdown_mode: i64,
    pub owner: String,
    pub ods_major: i64,
    pub ods_minor: i64,
    pub server_version: String,
    pub reads: i64,
    pub writes: i64,
    pub fetches: i64,
    pub marks: i64,
}

/// One live attachment from `MON$ATTACHMENTS`.
pub struct AttachmentInfo {
    pub id: i64,
    pub user: String,
    pub role: String,
    pub remote_address: String,
    pub remote_process: String,
    pub character_set: String,
    pub state: i64,
    pub timestamp: String,
    pub is_self: bool,
}

/// One live statement from `MON$STATEMENTS`.
pub struct StatementInfo {
    pub id: i64,
    pub attachment_id: i64,
    pub state: i64,
    pub timestamp: String,
    pub sql_text: String,
}

/// Full database statistics snapshot.
pub struct DatabaseStats {
    pub database: MonDatabase,
    pub attachments: Vec<AttachmentInfo>,
    pub statements: Vec<StatementInfo>,
}

/// Reads the `MON$DATABASE` row joined with `MON$IO_STATS`.
fn read_mon_database(client: &mut FirebirdClient) -> Result<MonDatabase, FbError> {
    let sql = "SELECT \
        TRIM(d.MON$DATABASE_NAME), \
        d.MON$PAGE_SIZE, d.MON$PAGES, \
        d.MON$OLDEST_TRANSACTION, d.MON$OLDEST_ACTIVE, d.MON$OLDEST_SNAPSHOT, \
        d.MON$NEXT_TRANSACTION, d.MON$SWEEP_INTERVAL, \
        d.MON$FORCED_WRITES, d.MON$READ_ONLY, d.MON$SQL_DIALECT, \
        CAST(d.MON$CREATION_DATE AS VARCHAR(64)), \
        COALESCE(d.MON$BACKUP_STATE, 0), \
        COALESCE(d.MON$SHUTDOWN_MODE, 0), \
        COALESCE(TRIM(d.MON$OWNER), ''), \
        d.MON$ODS_MAJOR, d.MON$ODS_MINOR, \
        CAST(rdb$get_context('SYSTEM', 'ENGINE_VERSION') AS VARCHAR(64)), \
        COALESCE(i.MON$PAGE_READS, 0), COALESCE(i.MON$PAGE_WRITES, 0), \
        COALESCE(i.MON$PAGE_FETCHES, 0), COALESCE(i.MON$PAGE_MARKS, 0) \
        FROM MON$DATABASE d \
        LEFT JOIN MON$IO_STATS i ON i.MON$STAT_ID = d.MON$STAT_ID";
    let result = query::query(client, sql)?;
    let row = result
        .rows
        .first()
        .ok_or_else(|| FbError::Other("MON$DATABASE returned no rows".into()))?;
    Ok(MonDatabase {
        name: cell_text(row, 0),
        page_size: cell_int(row, 1),
        pages: cell_int(row, 2),
        oldest_transaction: cell_int(row, 3),
        oldest_active: cell_int(row, 4),
        oldest_snapshot: cell_int(row, 5),
        next_transaction: cell_int(row, 6),
        sweep_interval: cell_int(row, 7),
        forced_writes: cell_int(row, 8) != 0,
        read_only: cell_int(row, 9) != 0,
        sql_dialect: cell_int(row, 10),
        creation_date: cell_text(row, 11),
        backup_state: cell_int(row, 12),
        shutdown_mode: cell_int(row, 13),
        owner: cell_text(row, 14),
        ods_major: cell_int(row, 15),
        ods_minor: cell_int(row, 16),
        server_version: cell_text(row, 17),
        reads: cell_int(row, 18),
        writes: cell_int(row, 19),
        fetches: cell_int(row, 20),
        marks: cell_int(row, 21),
    })
}

/// Reads all live attachments from `MON$ATTACHMENTS`.
fn read_mon_attachments(client: &mut FirebirdClient) -> Result<Vec<AttachmentInfo>, FbError> {
    let sql = "SELECT \
        a.MON$ATTACHMENT_ID, \
        COALESCE(TRIM(a.MON$USER), ''), \
        COALESCE(TRIM(a.MON$ROLE), ''), \
        COALESCE(TRIM(a.MON$REMOTE_ADDRESS), ''), \
        COALESCE(TRIM(a.MON$REMOTE_PROCESS), ''), \
        COALESCE(TRIM(c.RDB$CHARACTER_SET_NAME), ''), \
        COALESCE(a.MON$STATE, 0), \
        CAST(a.MON$TIMESTAMP AS VARCHAR(64)), \
        CASE WHEN a.MON$ATTACHMENT_ID = CURRENT_CONNECTION THEN 1 ELSE 0 END \
        FROM MON$ATTACHMENTS a \
        LEFT JOIN RDB$CHARACTER_SETS c \
        ON c.RDB$CHARACTER_SET_ID = a.MON$CHARACTER_SET_ID \
        ORDER BY a.MON$ATTACHMENT_ID";
    let result = query::query(client, sql)?;
    Ok(result
        .rows
        .iter()
        .map(|row| AttachmentInfo {
            id: cell_int(row, 0),
            user: cell_text(row, 1),
            role: cell_text(row, 2),
            remote_address: cell_text(row, 3),
            remote_process: cell_text(row, 4),
            character_set: cell_text(row, 5),
            state: cell_int(row, 6),
            timestamp: cell_text(row, 7),
            is_self: cell_int(row, 8) == 1,
        })
        .collect())
}

/// Reads all live statements from `MON$STATEMENTS`.
fn read_mon_statements(client: &mut FirebirdClient) -> Result<Vec<StatementInfo>, FbError> {
    let sql = "SELECT \
        s.MON$STATEMENT_ID, s.MON$ATTACHMENT_ID, \
        COALESCE(s.MON$STATE, 0), \
        CAST(s.MON$TIMESTAMP AS VARCHAR(64)), \
        COALESCE(s.MON$SQL_TEXT, '') \
        FROM MON$STATEMENTS s \
        ORDER BY s.MON$ATTACHMENT_ID, s.MON$STATEMENT_ID";
    let result = query::query(client, sql)?;
    Ok(result
        .rows
        .iter()
        .map(|row| StatementInfo {
            id: cell_int(row, 0),
            attachment_id: cell_int(row, 1),
            state: cell_int(row, 2),
            timestamp: cell_text(row, 3),
            sql_text: cell_text(row, 4),
        })
        .collect())
}

/// Reads the full database statistics snapshot.
pub fn database_stats(client: &mut FirebirdClient) -> Result<DatabaseStats, FbError> {
    let database = read_mon_database(client)?;
    let attachments = read_mon_attachments(client)?;
    let statements = read_mon_statements(client)?;
    Ok(DatabaseStats {
        database,
        attachments,
        statements,
    })
}

/// One row of the query history (active statements from `MON$STATEMENTS`
/// joined with `MON$ATTACHMENTS` for user/remote info).
pub struct QueryHistoryEntry {
    pub statement_id: i64,
    pub attachment_id: i64,
    pub user: String,
    pub remote_address: String,
    pub state: i64,
    pub timestamp: String,
    pub sql_text: String,
}

/// Reads the query history: all active statements from `MON$STATEMENTS`,
/// joined with `MON$ATTACHMENTS` for user and remote address.
///
/// `state_filter` optionally filters by statement state (0=idle, 1=active,
/// 2=stalled). `user_filter` optionally filters by user name (case-
/// insensitive). `limit` caps the number of rows returned (default 100,
/// max 1000).
pub fn query_history(
    client: &mut FirebirdClient,
    state_filter: Option<i64>,
    user_filter: Option<&str>,
    limit: u32,
) -> Result<Vec<QueryHistoryEntry>, FbError> {
    let mut sql = String::from(
        "SELECT \n"
        .to_string(),
    );
    sql.push_str("    s.MON$STATEMENT_ID, s.MON$ATTACHMENT_ID, \n");
    sql.push_str("    COALESCE(TRIM(a.MON$USER), ''), \n");
    sql.push_str("    COALESCE(TRIM(a.MON$REMOTE_ADDRESS), ''), \n");
    sql.push_str("    COALESCE(s.MON$STATE, 0), \n");
    sql.push_str("    CAST(s.MON$TIMESTAMP AS VARCHAR(64)), \n");
    sql.push_str("    COALESCE(s.MON$SQL_TEXT, '') \n");
    sql.push_str("    FROM MON$STATEMENTS s \n");
    sql.push_str("    LEFT JOIN MON$ATTACHMENTS a ON a.MON$ATTACHMENT_ID = s.MON$ATTACHMENT_ID \n");

    let mut conditions = Vec::new();
    if let Some(state) = state_filter {
        conditions.push(format!("s.MON$STATE = {state}"));
    }
    if let Some(user) = user_filter {
        // Escape single quotes in the user name to prevent SQL injection.
        let escaped = user.replace('\'', "''");
        conditions.push(format!(
            "UPPER(TRIM(a.MON$USER)) = UPPER('{}')",
            escaped
        ));
    }
    if !conditions.is_empty() {
        sql.push_str("    WHERE ");
        sql.push_str(&conditions.join(" AND "));
        sql.push('\n');
    }

    sql.push_str("    ORDER BY s.MON$ATTACHMENT_ID, s.MON$STATEMENT_ID");

    let result = query::query_with_limit(client, &sql, limit, 0)?;
    Ok(result
        .rows
        .iter()
        .map(|row| QueryHistoryEntry {
            statement_id: cell_int(row, 0),
            attachment_id: cell_int(row, 1),
            user: cell_text(row, 2),
            remote_address: cell_text(row, 3),
            state: cell_int(row, 4),
            timestamp: cell_text(row, 5),
            sql_text: cell_text(row, 6),
        })
        .collect())
}

// --- helpers ---

fn cell_text(row: &[Value], idx: usize) -> String {
    match row.get(idx) {
        Some(Value::Text(s)) => s.clone(),
        Some(Value::Int(n)) => n.to_string(),
        Some(Value::Float(f)) => f.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn cell_int(row: &[Value], idx: usize) -> i64 {
    match row.get(idx) {
        Some(Value::Int(n)) => *n,
        Some(Value::Text(s)) => s.parse().unwrap_or(0),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => 0,
    }
}

// Extend Value with convenience accessors used above.
impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Text(s) => s.parse().ok(),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<String> {
        match self {
            Value::Text(s) => Some(s.clone()),
            Value::Int(n) => Some(n.to_string()),
            _ => None,
        }
    }
}
