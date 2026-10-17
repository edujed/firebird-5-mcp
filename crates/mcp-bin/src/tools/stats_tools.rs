//! Statistics and encryption-state tool handlers.

use firebird_5_mcp_core::{Config, stats};
use serde_json::Value;

use super::common::*;

/// Returns the at-rest encryption state of the database.
pub fn handle_crypt_state(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let state = stats::crypt_state(&mut client).map_err(|e| e.to_string())?;
    let version = stats::engine_version(&mut client).map_err(|e| e.to_string())?;

    let mut msg = format!("Encryption state: {}\n", state.label());
    msg.push_str(&format!("Engine version: {version}\n"));
    if state.is_encrypted() {
        msg.push_str("The database file is encrypted at rest.\n");
    } else if matches!(state, stats::CryptState::Unencrypted) {
        msg.push_str("The database file is not encrypted at rest.\n");
    }
    Ok(msg)
}

/// Returns a snapshot of database statistics (MON$DATABASE, MON$ATTACHMENTS,
/// MON$STATEMENTS).
pub fn handle_database_stats(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let snap = stats::database_stats(&mut client).map_err(|e| e.to_string())?;
    let db = &snap.database;

    let mut b = String::new();
    b.push_str("### Database Statistics\n\n");
    b.push_str(&format!("**Name:** {}\n", db.name));
    b.push_str(&format!("**Engine:** {}\n", db.server_version));
    b.push_str(&format!("**Page size:** {}\n", db.page_size));
    b.push_str(&format!("**Pages:** {}\n", db.pages));
    b.push_str(&format!(
        "**Size (approx):** {:.1} MB\n",
        (db.pages as f64 * db.page_size as f64) / (1024.0 * 1024.0)
    ));
    b.push_str(&format!("**SQL dialect:** {}\n", db.sql_dialect));
    b.push_str(&format!("**Owner:** {}\n", db.owner));
    b.push_str(&format!("**Created:** {}\n", db.creation_date));
    b.push_str(&format!(
        "**Read-only:** {}\n",
        if db.read_only { "yes" } else { "no" }
    ));
    b.push_str(&format!(
        "**Forced writes:** {}\n",
        if db.forced_writes { "yes" } else { "no" }
    ));
    b.push_str(&format!("**ODS:** {}.{}\n", db.ods_major, db.ods_minor));
    b.push_str(&format!(
        "**Backup state:** {}\n",
        backup_state_label(db.backup_state)
    ));
    b.push_str(&format!(
        "**Shutdown mode:** {}\n",
        shutdown_mode_label(db.shutdown_mode)
    ));

    b.push_str("\n### Transactions\n\n");
    b.push_str(&format!(
        "Oldest transaction (OIT): {}\n",
        db.oldest_transaction
    ));
    b.push_str(&format!("Oldest active (OAT): {}\n", db.oldest_active));
    b.push_str(&format!("Oldest snapshot (OST): {}\n", db.oldest_snapshot));
    b.push_str(&format!("Next transaction: {}\n", db.next_transaction));
    b.push_str(&format!("Sweep interval: {}\n", db.sweep_interval));

    b.push_str("\n### I/O (cumulative)\n\n");
    b.push_str(&format!("Page reads: {}\n", db.reads));
    b.push_str(&format!("Page writes: {}\n", db.writes));
    b.push_str(&format!("Page fetches: {}\n", db.fetches));
    b.push_str(&format!("Page marks: {}\n", db.marks));

    // Attachments
    b.push_str(&format!(
        "\n### Attachments ({})\n\n",
        snap.attachments.len()
    ));
    if snap.attachments.is_empty() {
        b.push_str("None.\n");
    } else {
        b.push_str("ID | USER | ROLE | REMOTE | PROCESS | CHARSET | STATE | TIMESTAMP | SELF\n");
        b.push_str(&["---".repeat(9)].join(" | "));
        b.push('\n');
        for a in &snap.attachments {
            b.push_str(&format!(
                "{} | {} | {} | {} | {} | {} | {} | {} | {}\n",
                a.id,
                a.user,
                a.role,
                a.remote_address,
                a.remote_process,
                a.character_set,
                attachment_state_label(a.state),
                a.timestamp,
                if a.is_self { "yes" } else { "no" }
            ));
        }
    }

    // Statements
    b.push_str(&format!(
        "\n### Active Statements ({})\n\n",
        snap.statements.len()
    ));
    if snap.statements.is_empty() {
        b.push_str("None.\n");
    } else {
        b.push_str("ID | ATTACHMENT | STATE | TIMESTAMP | SQL\n");
        b.push_str("----|------------|-------|-----------|----\n");
        for s in &snap.statements {
            let sql_trunc = truncate(&s.sql_text, 120);
            b.push_str(&format!(
                "{} | {} | {} | {} | {}\n",
                s.id,
                s.attachment_id,
                statement_state_label(s.state),
                s.timestamp,
                sql_trunc
            ));
        }
    }

    Ok(b)
}

fn backup_state_label(v: i64) -> &'static str {
    match v {
        0 => "normal",
        1 => "stalled",
        2 => "merge",
        _ => "unknown",
    }
}

fn shutdown_mode_label(v: i64) -> &'static str {
    match v {
        0 => "online",
        1 => "multi",
        2 => "single",
        3 => "full",
        _ => "unknown",
    }
}

fn attachment_state_label(v: i64) -> &'static str {
    match v {
        0 => "idle",
        1 => "active",
        2 => "stalled",
        _ => "unknown",
    }
}

fn statement_state_label(v: i64) -> &'static str {
    match v {
        0 => "idle",
        1 => "active",
        2 => "stalled",
        _ => "unknown",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

/// Returns the query history: active statements from `MON$STATEMENTS`,
/// joined with `MON$ATTACHMENTS` for user and remote address.
pub fn handle_query_history(cfg: &Config, args: &Value) -> Result<String, String> {
    let state_filter = arg_str(args, "state").and_then(|s| s.parse::<i64>().ok());
    let user_filter = arg_str(args, "user")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let limit = arg_str(args, "limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(100)
        .min(1000);

    let mut client = open_client(cfg, args)?;
    let entries = stats::query_history(&mut client, state_filter, user_filter.as_deref(), limit)
        .map_err(|e| e.to_string())?;

    let mut b = String::new();
    b.push_str(&format!(
        "### Query History ({} statements)\n\n",
        entries.len()
    ));

    if let Some(state) = state_filter {
        b.push_str(&format!(
            "Filter: state = {} ({}), ",
            state,
            statement_state_label(state)
        ));
    }
    if let Some(ref user) = user_filter {
        b.push_str(&format!("user = '{}', ", user));
    }
    b.push_str(&format!("limit = {}\n\n", limit));

    if entries.is_empty() {
        b.push_str("No statements found.\n");
        return Ok(b);
    }

    b.push_str("STMT_ID | ATTACH | USER | REMOTE | STATE | TIMESTAMP | SQL\n");
    b.push_str("--------|--------|------|--------|-------|-----------|----\n");
    for e in &entries {
        let sql_trunc = truncate(&e.sql_text, 100);
        b.push_str(&format!(
            "{} | {} | {} | {} | {} | {} | {}\n",
            e.statement_id,
            e.attachment_id,
            e.user,
            e.remote_address,
            statement_state_label(e.state),
            e.timestamp,
            sql_trunc
        ));
    }

    Ok(b)
}
