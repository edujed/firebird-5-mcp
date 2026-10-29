//! Schema introspection via RDB$ system tables.
//! Ported from the Go implementation (`internal/firebird/client.go`).

use rsfbclient::FbError;

use crate::metadata::decode_blr_default;
use mcp_core::FirebirdClient;
use mcp_core::query::{query, query_with_param};
use mcp_types::Value;

pub mod markdown;

pub use markdown::format_schema_markdown;

/// A table or view in the database.
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub r#type: String,
    pub comment: String,
}

/// A column in a table.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub type_num: i64,
    pub sub_type: i64,
    pub size: i64,
    pub precision: i64,
    pub scale: i64,
    pub nullable: Option<i64>,
    pub default: Option<String>,
    pub position: Option<i64>,
    pub domain: String,
}

/// An index on a table.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
    pub r#type: Option<i64>,
    pub columns: String,
}

/// A constraint on a table.
#[derive(Debug, Clone)]
pub struct ConstraintInfo {
    pub name: String,
    pub r#type: String,
}

/// A trigger on a table.
#[derive(Debug, Clone)]
pub struct TriggerInfo {
    pub name: String,
    pub r#type: Option<i64>,
    pub sequence: Option<i64>,
}

/// The full schema of a table or view.
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub table: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub constraints: Vec<ConstraintInfo>,
    pub triggers: Vec<TriggerInfo>,
    pub comments: std::collections::HashMap<String, String>,
}

/// A foreign key constraint.
#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub table: String,
    pub column: String,
    pub ref_table: String,
    pub ref_column: String,
}

/// Returns all user tables and views.
pub fn list_tables(client: &mut FirebirdClient) -> Result<Vec<TableInfo>, FbError> {
    let sql = r#"
        SELECT
            R.RDB$RELATION_NAME AS NAME,
            CASE
                WHEN R.RDB$VIEW_BLR IS NULL THEN 'TABLE'
                ELSE 'VIEW'
            END AS TYPE
        FROM RDB$RELATIONS R
        WHERE R.RDB$SYSTEM_FLAG = 0
        ORDER BY R.RDB$RELATION_NAME
    "#;
    let result = query(client, sql)?;
    let mut tables = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        tables.push(TableInfo {
            name: text_at(row, 0),
            r#type: text_at(row, 1),
            comment: String::new(),
        });
    }
    Ok(tables)
}

/// Returns the full schema of a table or view.
pub fn describe_table(
    client: &mut FirebirdClient,
    table_name: &str,
) -> Result<TableSchema, FbError> {
    let mut schema = TableSchema {
        table: table_name.to_string(),
        columns: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::new(),
        triggers: Vec::new(),
        comments: std::collections::HashMap::new(),
    };

    // Columns (join RDB$FIELDS to get type info)
    let cols_sql = r#"
        SELECT
            F.RDB$FIELD_NAME        AS NAME,
            T.RDB$FIELD_TYPE        AS TYPE_NUM,
            T.RDB$FIELD_SUB_TYPE    AS SUB_TYPE,
            T.RDB$FIELD_LENGTH      AS SIZE,
            T.RDB$FIELD_PRECISION   AS PREC,
            T.RDB$FIELD_SCALE       AS SCALE,
            F.RDB$NULL_FLAG         AS NULL_FLAG,
            CAST(F.RDB$DEFAULT_VALUE AS BLOB SUB_TYPE 1) AS DEFVAL,
            F.RDB$FIELD_POSITION    AS POS,
            F.RDB$FIELD_SOURCE      AS DOMAIN
        FROM RDB$RELATION_FIELDS F
        LEFT JOIN RDB$FIELDS T
            ON T.RDB$FIELD_NAME = F.RDB$FIELD_SOURCE
        WHERE F.RDB$RELATION_NAME = ?
        ORDER BY F.RDB$FIELD_POSITION
    "#;
    let cols_result = query_with_param(client, cols_sql, table_name)?;
    for row in &cols_result.rows {
        let mut domain = text_at(row, 9);
        if domain.starts_with("RDB$") {
            domain = String::new();
        }
        schema.columns.push(ColumnInfo {
            name: text_at(row, 0),
            type_num: int_at(row, 1),
            sub_type: int_at(row, 2),
            size: int_at(row, 3),
            precision: int_at(row, 4),
            scale: int_at(row, 5),
            nullable: opt_int_at(row, 6),
            default: opt_text_at(row, 7).and_then(|raw| decode_blr_default(&raw)),
            position: opt_int_at(row, 8),
            domain,
        });
    }

    // Comments (RDB$OBJECT_COMMENTS may not exist in all Firebird versions)
    if let Ok(comments) = get_comments(client, table_name) {
        schema.comments = comments;
    }

    // Indexes
    let idx_sql = r#"
        SELECT
            I.RDB$INDEX_NAME           AS NAME,
            I.RDB$UNIQUE_FLAG          AS IS_UNIQUE,
            I.RDB$INDEX_TYPE           AS TYPE,
            S.RDB$FIELD_NAME           AS COLUMN_NAME,
            S.RDB$FIELD_POSITION       AS COL_POS
        FROM RDB$INDICES I
        JOIN RDB$INDEX_SEGMENTS S
            ON S.RDB$INDEX_NAME = I.RDB$INDEX_NAME
        WHERE I.RDB$RELATION_NAME = ?
        ORDER BY I.RDB$INDEX_NAME, S.RDB$FIELD_POSITION
    "#;
    let idx_result = query_with_param(client, idx_sql, table_name)?;
    // Group columns by index name
    let mut idx_map: std::collections::HashMap<String, IndexInfo> =
        std::collections::HashMap::new();
    let mut idx_order: Vec<String> = Vec::new();
    for row in &idx_result.rows {
        let name = text_at(row, 0);
        let unique = int_at(row, 1) == 1;
        let idx_type = opt_int_at(row, 2);
        let col_name = text_at(row, 3);
        if let Some(idx) = idx_map.get_mut(&name) {
            idx.columns.push_str(", ");
            idx.columns.push_str(&col_name);
        } else {
            idx_map.insert(
                name.clone(),
                IndexInfo {
                    name: name.clone(),
                    unique,
                    r#type: idx_type,
                    columns: col_name,
                },
            );
            idx_order.push(name);
        }
    }
    for name in idx_order {
        if let Some(idx) = idx_map.remove(&name) {
            schema.indexes.push(idx);
        }
    }

    // Constraints
    let con_sql = r#"
        SELECT
            C.RDB$CONSTRAINT_NAME        AS NAME,
            C.RDB$CONSTRAINT_TYPE        AS TYPE
        FROM RDB$RELATION_CONSTRAINTS C
        WHERE C.RDB$RELATION_NAME = ?
        ORDER BY C.RDB$CONSTRAINT_NAME
    "#;
    let con_result = query_with_param(client, con_sql, table_name)?;
    for row in &con_result.rows {
        schema.constraints.push(ConstraintInfo {
            name: text_at(row, 0),
            r#type: text_at(row, 1),
        });
    }

    // Triggers
    let trg_sql = r#"
        SELECT
            T.RDB$TRIGGER_NAME     AS NAME,
            T.RDB$TRIGGER_TYPE     AS TYPE,
            T.RDB$TRIGGER_SEQUENCE AS SEQUENCE
        FROM RDB$TRIGGERS T
        WHERE T.RDB$RELATION_NAME = ?
        ORDER BY T.RDB$TRIGGER_SEQUENCE
    "#;
    let trg_result = query_with_param(client, trg_sql, table_name)?;
    for row in &trg_result.rows {
        schema.triggers.push(TriggerInfo {
            name: text_at(row, 0),
            r#type: opt_int_at(row, 1),
            sequence: opt_int_at(row, 2),
        });
    }

    Ok(schema)
}

/// Returns all foreign keys in the database.
pub fn get_foreign_keys(client: &mut FirebirdClient) -> Result<Vec<ForeignKeyInfo>, FbError> {
    let sql = r#"
        SELECT
            c.RDB$CONSTRAINT_NAME       AS CONSTRAINT_NAME,
            c.RDB$RELATION_NAME         AS TABLE_NAME,
            pk.RDB$FIELD_NAME           AS FK_COLUMN,
            fc.RDB$RELATION_NAME        AS REF_TABLE,
            rc.RDB$FIELD_NAME           AS REF_COLUMN
        FROM RDB$RELATION_CONSTRAINTS c
        JOIN RDB$INDICES ci ON ci.RDB$INDEX_NAME = c.RDB$INDEX_NAME
        JOIN RDB$INDEX_SEGMENTS pk ON pk.RDB$INDEX_NAME = ci.RDB$INDEX_NAME
        LEFT JOIN RDB$INDICES ri ON ri.RDB$INDEX_NAME = ci.RDB$FOREIGN_KEY
        LEFT JOIN RDB$INDEX_SEGMENTS rc ON rc.RDB$INDEX_NAME = ri.RDB$INDEX_NAME
            AND rc.RDB$FIELD_POSITION = pk.RDB$FIELD_POSITION
        LEFT JOIN RDB$RELATION_CONSTRAINTS fc ON fc.RDB$INDEX_NAME = ci.RDB$FOREIGN_KEY
        WHERE c.RDB$CONSTRAINT_TYPE = 'FOREIGN KEY'
        ORDER BY c.RDB$RELATION_NAME, c.RDB$CONSTRAINT_NAME, pk.RDB$FIELD_POSITION
    "#;
    let result = query(client, sql)?;
    let mut fks = Vec::with_capacity(result.rows.len());
    for row in &result.rows {
        fks.push(ForeignKeyInfo {
            constraint_name: text_at(row, 0),
            table: text_at(row, 1),
            column: text_at(row, 2),
            ref_table: text_at(row, 3),
            ref_column: text_at(row, 4),
        });
    }
    Ok(fks)
}

/// Fetches table and column comments if RDB$OBJECT_COMMENTS exists.
fn get_comments(
    client: &mut FirebirdClient,
    table_name: &str,
) -> Result<std::collections::HashMap<String, String>, FbError> {
    // Check if RDB$OBJECT_COMMENTS exists
    let check_sql =
        "SELECT COUNT(*) FROM RDB$RELATIONS WHERE RDB$RELATION_NAME = 'RDB$OBJECT_COMMENTS'";
    let check_result = query(client, check_sql)?;
    let count_val = check_result
        .rows
        .first()
        .and_then(|r| match r.first() {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    if count_val == 0 {
        return Ok(std::collections::HashMap::new());
    }

    let comment_sql = r#"
        SELECT
            RDB$RELATION_NAME AS REL_NAME,
            RDB$FIELD_NAME    AS FIELD_NAME,
            RDB$OBJECT_COMMENT AS COMMENT
        FROM RDB$OBJECT_COMMENTS
        WHERE RDB$RELATION_NAME = ?
    "#;
    let result = query_with_param(client, comment_sql, table_name)?;
    let mut comments = std::collections::HashMap::new();
    for row in &result.rows {
        let field_name = text_at(row, 1);
        let comment = text_at(row, 2);
        if field_name.is_empty() {
            comments.insert("table".to_string(), comment);
        } else {
            comments.insert(field_name, comment);
        }
    }
    Ok(comments)
}

// --- Row value helpers ---

/// Returns the value at `idx` as a trimmed String, or empty string if NULL.
fn text_at(row: &[Value], idx: usize) -> String {
    row.get(idx)
        .map(|v| match v {
            Value::Text(s) => s.trim().to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

/// Returns the value at `idx` as an i64, or 0 if NULL or non-numeric.
fn int_at(row: &[Value], idx: usize) -> i64 {
    row.get(idx)
        .and_then(|v| match v {
            Value::Int(n) => Some(*n),
            Value::Text(s) => s.parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0)
}

/// Returns the value at `idx` as an i64, or None if NULL or non-numeric.
fn opt_int_at(row: &[Value], idx: usize) -> Option<i64> {
    row.get(idx).and_then(|v| match v {
        Value::Int(n) => Some(*n),
        Value::Text(s) => s.parse::<i64>().ok(),
        _ => None,
    })
}

/// Returns the value at `idx` as a trimmed String, or None if NULL or empty.
fn opt_text_at(row: &[Value], idx: usize) -> Option<String> {
    row.get(idx).and_then(|v| match v {
        Value::Text(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    })
}
