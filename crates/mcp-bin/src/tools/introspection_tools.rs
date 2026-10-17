//! Introspection tools: table stats, users, grants, blob info, index usage.
//!
//! These are read-only tools that query the RDB$ system tables to provide
//! database health and structure insights.

use firebird_5_mcp_core::{Config, query};
use serde_json::Value;

use super::common::{arg_str, open_client, result_to_text};

/// Returns per-table row counts.
///
/// For a single table, uses a simple `SELECT COUNT(*) FROM <table>`.
/// For all tables, uses an `EXECUTE BLOCK` with `EXECUTE STATEMENT` to
/// dynamically count each table (Firebird does not support dynamic table
/// references in subqueries).
///
/// For large tables the count can be slow; the tool is intended for
/// development/analysis use.
pub fn handle_table_stats(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let table = arg_str(args, "table");

    let (sql, title) = match &table {
        Some(t) => {
            let t = t.to_uppercase();
            (
                format!(
                    "SELECT \
                     r.RDB$RELATION_NAME AS TABLE_NAME, \
                     (SELECT COUNT(*) FROM {t}) AS ROW_COUNT \
                     FROM RDB$RELATIONS r \
                     WHERE r.RDB$RELATION_NAME = '{t}' \
                       AND r.RDB$VIEW_BLR IS NULL"
                ),
                format!("Table Stats: {t}"),
            )
        }
        None => (
            "EXECUTE BLOCK\n\
             RETURNS (TABLE_NAME VARCHAR(31), TOTAL BIGINT)\n\
             AS\n\
             DECLARE VARIABLE SQL_TXT VARCHAR(200);\n\
             BEGIN\n\
                 FOR SELECT TRIM(RDB$RELATION_NAME)\n\
                     FROM RDB$RELATIONS\n\
                     WHERE COALESCE(RDB$SYSTEM_FLAG, 0) = 0\n\
                       AND RDB$VIEW_BLR IS NULL\n\
                     ORDER BY RDB$RELATION_NAME\n\
                     INTO :TABLE_NAME\n\
                 DO\n\
                 BEGIN\n\
                     SQL_TXT = 'SELECT COUNT(*) FROM \"' || TRIM(:TABLE_NAME) || '\"';\n\
                     EXECUTE STATEMENT SQL_TXT INTO :TOTAL;\n\
                     SUSPEND;\n\
                 END\n\
             END"
            .to_string(),
            "Table Stats (all tables)".to_string(),
        ),
    };

    let result = query::query(&mut client, &sql).map_err(|e| e.to_string())?;
    let mut b = String::new();
    b.push_str(&format!("## {title}\n\n"));
    b.push_str(&result_to_text(&result));
    Ok(b)
}

/// Lists all users with access to the database.
///
/// Combines `RDB$DB_CREATORS` (database owners) with `RDB$USER_PRIVILEGES`
/// (users with explicit grants) to provide a complete picture of who has
/// access to the database.
pub fn handle_users(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let sql = "SELECT \
        COALESCE(c.RDB$USER, p.RDB$USER) AS USER_NAME, \
        c.RDB$USER_TYPE AS CREATOR_TYPE, \
        p.RDB$PRIVILEGE, \
        p.RDB$GRANT_OPTION \
        FROM RDB$DB_CREATORS c \
        FULL OUTER JOIN RDB$USER_PRIVILEGES p \
            ON c.RDB$USER = p.RDB$USER \
        ORDER BY USER_NAME, p.RDB$PRIVILEGE";
    let result = query::query(&mut client, sql).map_err(|e| e.to_string())?;
    let mut b = String::new();
    b.push_str("## Database Users\n\n");
    b.push_str(&result_to_text(&result));
    b.push_str("\n| CREATOR_TYPE | Meaning |\n|---|---|\n");
    b.push_str("| 0 | Database owner |\n");
    b.push_str("| 1 | User with privileges |\n");
    Ok(b)
}

/// Shows GRANTs for a specific table or for all tables.
///
/// Uses `RDB$USER_PRIVILEGES` which stores explicit grants in all Firebird
/// versions (2.5+).
pub fn handle_grants(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let table = arg_str(args, "table");

    let sql = match &table {
        Some(t) => {
            let t = t.to_uppercase();
            format!(
                "SELECT \
                 p.RDB$RELATION_NAME, \
                 p.RDB$USER AS GRANTEE, \
                 p.RDB$PRIVILEGE, \
                 p.RDB$GRANT_OPTION \
                 FROM RDB$USER_PRIVILEGES p \
                 WHERE p.RDB$RELATION_NAME = '{t}' \
                 ORDER BY p.RDB$USER, p.RDB$PRIVILEGE"
            )
        }
        None => "SELECT \
                p.RDB$RELATION_NAME, \
                p.RDB$USER AS GRANTEE, \
                p.RDB$PRIVILEGE, \
                p.RDB$GRANT_OPTION \
                FROM RDB$USER_PRIVILEGES p \
                ORDER BY p.RDB$RELATION_NAME, p.RDB$USER, p.RDB$PRIVILEGE"
            .to_string(),
    };

    let result = query::query(&mut client, &sql).map_err(|e| e.to_string())?;
    let title = match &table {
        Some(t) => format!("Grants for {t}"),
        None => "Grants (all tables)".to_string(),
    };
    let mut b = String::new();
    b.push_str(&format!("## {title}\n\n"));
    if result.rows.is_empty() {
        b.push_str("No explicit grants found (tables use default permissions).\n");
    } else {
        b.push_str(&result_to_text(&result));
    }
    Ok(b)
}

/// Shows BLOB column information: table, column, blob subtype, and length.
///
/// Joins `RDB$RELATION_FIELDS` with `RDB$FIELDS` to get the field type and
/// subtype (BLOB sub_type 1 = TEXT, 2 = BINARY).
pub fn handle_blob_info(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let table = arg_str(args, "table");

    let sql = match &table {
        Some(t) => {
            let t = t.to_uppercase();
            format!(
                "SELECT \
                 f.RDB$RELATION_NAME, \
                 f.RDB$FIELD_NAME, \
                 t.RDB$FIELD_TYPE, \
                 t.RDB$FIELD_SUB_TYPE, \
                 t.RDB$FIELD_LENGTH, \
                 CASE t.RDB$FIELD_SUB_TYPE \
                   WHEN 1 THEN 'TEXT' \
                   WHEN 2 THEN 'BINARY' \
                   ELSE 'UNKNOWN' \
                 END AS BLOB_TYPE \
                 FROM RDB$RELATION_FIELDS f \
                 JOIN RDB$FIELDS t ON f.RDB$FIELD_SOURCE = t.RDB$FIELD_NAME \
                 WHERE t.RDB$FIELD_TYPE IN (261, 45) \
                   AND f.RDB$RELATION_NAME = '{t}' \
                 ORDER BY f.RDB$FIELD_POSITION"
            )
        }
        None => "SELECT \
                f.RDB$RELATION_NAME, \
                f.RDB$FIELD_NAME, \
                t.RDB$FIELD_TYPE, \
                t.RDB$FIELD_SUB_TYPE, \
                t.RDB$FIELD_LENGTH, \
                CASE t.RDB$FIELD_SUB_TYPE \
                  WHEN 1 THEN 'TEXT' \
                  WHEN 2 THEN 'BINARY' \
                  ELSE 'UNKNOWN' \
                END AS BLOB_TYPE \
                FROM RDB$RELATION_FIELDS f \
                JOIN RDB$FIELDS t ON f.RDB$FIELD_SOURCE = t.RDB$FIELD_NAME \
                WHERE t.RDB$FIELD_TYPE IN (261, 45) \
                  AND COALESCE(f.RDB$SYSTEM_FLAG, 0) = 0 \
                ORDER BY f.RDB$RELATION_NAME, f.RDB$FIELD_POSITION"
            .to_string(),
    };

    let result = query::query(&mut client, &sql).map_err(|e| e.to_string())?;
    let title = match &table {
        Some(t) => format!("BLOB Columns in {t}"),
        None => "BLOB Columns (all tables)".to_string(),
    };
    let mut b = String::new();
    b.push_str(&format!("## {title}\n\n"));
    if result.rows.is_empty() {
        b.push_str("No BLOB columns found.\n");
    } else {
        b.push_str(&result_to_text(&result));
        b.push_str(
            "\n> **Note:** For actual BLOB sizes, use `firebird_query` with \
                   `SELECT TABLE_NAME, COLUMN_NAME, OCTET_LENGTH(col) FROM ...` \
                   on specific tables.\n",
        );
    }
    Ok(b)
}

/// Shows index information: name, type, uniqueness, and columns.
///
/// Uses `RDB$INDICES` joined with `RDB$INDEX_SEGMENTS` to list all indexes
/// and their columns. The `LIST()` window function concatenates column
/// names in a single string per index.
pub fn handle_index_usage(cfg: &Config, args: &Value) -> Result<String, String> {
    let mut client = open_client(cfg, args)?;
    let table = arg_str(args, "table");

    let sql = match &table {
        Some(t) => {
            let t = t.to_uppercase();
            format!(
                "SELECT DISTINCT \
                 i.RDB$INDEX_NAME, \
                 i.RDB$RELATION_NAME, \
                 i.RDB$UNIQUE_FLAG, \
                 i.RDB$INDEX_TYPE, \
                 LIST(f.RDB$FIELD_NAME, ', ') OVER (PARTITION BY i.RDB$INDEX_NAME) AS COLUMNS \
                 FROM RDB$INDICES i \
                 JOIN RDB$INDEX_SEGMENTS f ON i.RDB$INDEX_NAME = f.RDB$INDEX_NAME \
                 WHERE i.RDB$RELATION_NAME = '{t}' \
                 ORDER BY i.RDB$INDEX_NAME"
            )
        }
        None => "SELECT DISTINCT \
                i.RDB$INDEX_NAME, \
                i.RDB$RELATION_NAME, \
                i.RDB$UNIQUE_FLAG, \
                i.RDB$INDEX_TYPE, \
                LIST(f.RDB$FIELD_NAME, ', ') OVER (PARTITION BY i.RDB$INDEX_NAME) AS COLUMNS \
                FROM RDB$INDICES i \
                JOIN RDB$INDEX_SEGMENTS f ON i.RDB$INDEX_NAME = f.RDB$INDEX_NAME \
                WHERE COALESCE(i.RDB$SYSTEM_FLAG, 0) = 0 \
                ORDER BY i.RDB$RELATION_NAME, i.RDB$INDEX_NAME"
            .to_string(),
    };

    let result = query::query(&mut client, &sql).map_err(|e| e.to_string())?;
    let title = match &table {
        Some(t) => format!("Indexes on {t}"),
        None => "Indexes (all tables)".to_string(),
    };
    let mut b = String::new();
    b.push_str(&format!("## {title}\n\n"));
    if result.rows.is_empty() {
        b.push_str("No indexes found.\n");
    } else {
        b.push_str(&result_to_text(&result));
        b.push_str("\n| UNIQUE_FLAG | Meaning |\n|---|---|\n");
        b.push_str("| 0 | Non-unique |\n");
        b.push_str("| 1 | Unique |\n");
        b.push_str("| 2 | Primary key |\n\n");
        b.push_str("| INDEX_TYPE | Meaning |\n|---|---|\n");
        b.push_str("| 0 | B-tree (default) |\n");
        b.push_str("| 1 | Full-text search |\n");
    }
    Ok(b)
}
