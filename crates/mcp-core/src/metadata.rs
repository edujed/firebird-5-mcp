//! Full DDL extraction (replaces `isql -x`).
//!
//! Generates a SQL script that recreates the database structure by querying
//! the RDB$ system tables: domains, tables (columns, constraints, indexes),
//! views, sequences, triggers, procedures, and functions.

use rsfbclient::FbError;

use crate::types::format_column_type;
use mcp_core::FirebirdClient;
use mcp_core::query::{query, query_with_param};
use mcp_types::Value;
use mcp_types::{is_not_null, trigger_type_label};

/// Extracts the complete DDL for the database.
pub fn extract_ddl(client: &mut FirebirdClient) -> Result<String, FbError> {
    extract_ddl_filtered(client, None)
}

/// Extracts DDL for specific objects only.
/// If `objects` is None, extracts everything.
pub fn extract_ddl_filtered(
    client: &mut FirebirdClient,
    objects: Option<&[String]>,
) -> Result<String, FbError> {
    let filter: Option<std::collections::HashSet<String>> =
        objects.map(|objs| objs.iter().map(|s| s.to_uppercase()).collect());

    let mut out = String::with_capacity(8192);
    out.push_str("-- Firebird DDL extracted by mcp-core\n");
    out.push_str("-- (replaces isql -x)\n\n");

    extract_domains(client, &mut out, &filter)?;
    extract_tables(client, &mut out, &filter)?;
    extract_views(client, &mut out, &filter)?;
    extract_sequences(client, &mut out, &filter)?;
    extract_triggers(client, &mut out, &filter)?;
    extract_procedures(client, &mut out, &filter)?;
    extract_functions(client, &mut out, &filter)?;

    Ok(out)
}

/// Returns `true` if the object name passes the extraction filter.
fn should_extract(filter: &Option<std::collections::HashSet<String>>, name: &str) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(&name.trim().to_uppercase()),
    }
}

/// Appends `CREATE DOMAIN` statements for all user domains to the output.
fn extract_domains(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT
            RDB$FIELD_NAME,
            RDB$FIELD_TYPE,
            RDB$FIELD_SUB_TYPE,
            RDB$FIELD_LENGTH,
            RDB$FIELD_PRECISION,
            RDB$FIELD_SCALE,
            RDB$DEFAULT_SOURCE,
            RDB$NULL_FLAG
        FROM RDB$FIELDS
        WHERE RDB$SYSTEM_FLAG = 0
          AND RDB$FIELD_NAME NOT LIKE 'RDB$%'
          AND RDB$FIELD_NAME NOT LIKE 'MON$%'
          AND RDB$FIELD_NAME NOT LIKE 'SEC$%'
        ORDER BY RDB$FIELD_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Domains\n\n");
            any = true;
        }
        let type_str = format_column_type(
            int_at(row, 1),
            int_at(row, 2),
            int_at(row, 3),
            int_at(row, 4),
            int_at(row, 5),
        );
        let default_raw = opt_text_at(row, 6).unwrap_or_default();
        let default = decode_blr_default(&default_raw).unwrap_or_default();
        // RDB$NULL_FLAG: 1 = NOT NULL, NULL/0 = nullable
        let null_flag = opt_int_at(row, 7);
        let mut stmt = format!("CREATE DOMAIN {name} AS {type_str}");
        if !default.is_empty() {
            stmt.push_str(&format!(" DEFAULT {default}"));
        }
        if is_not_null(null_flag) {
            stmt.push_str(" NOT NULL");
        }
        out.push_str(&format!("{stmt};\n"));
    }
    if any {
        out.push('\n');
    }
    Ok(())
}

/// Appends `CREATE TABLE` statements with columns, constraints, and indexes.
fn extract_tables(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let tables_sql = r#"
        SELECT RDB$RELATION_NAME
        FROM RDB$RELATIONS
        WHERE RDB$SYSTEM_FLAG = 0
          AND RDB$VIEW_BLR IS NULL
        ORDER BY RDB$RELATION_NAME
    "#;
    let tables = query(client, tables_sql)?;

    for row in &tables.rows {
        let table = text_at(row, 0);
        if !should_extract(filter, &table) {
            continue;
        }
        out.push_str(&format!("-- Table: {table}\n"));

        // Columns
        let cols_sql = r#"
            SELECT
                F.RDB$FIELD_NAME,
                T.RDB$FIELD_TYPE,
                T.RDB$FIELD_SUB_TYPE,
                T.RDB$FIELD_LENGTH,
                T.RDB$FIELD_PRECISION,
                T.RDB$FIELD_SCALE,
                F.RDB$NULL_FLAG,
                CAST(F.RDB$DEFAULT_VALUE AS BLOB SUB_TYPE 1) AS RDB$DEFAULT_VALUE,
                F.RDB$FIELD_SOURCE
            FROM RDB$RELATION_FIELDS F
            LEFT JOIN RDB$FIELDS T
                ON T.RDB$FIELD_NAME = F.RDB$FIELD_SOURCE
            WHERE F.RDB$RELATION_NAME = ?
            ORDER BY F.RDB$FIELD_POSITION
        "#;
        let cols = query_with_param(client, cols_sql, &table)?;
        out.push_str(&format!("CREATE TABLE {table} (\n"));
        let mut col_defs: Vec<String> = Vec::new();
        for c in &cols.rows {
            let name = text_at(c, 0);
            let type_str = format_column_type(
                int_at(c, 1),
                int_at(c, 2),
                int_at(c, 3),
                int_at(c, 4),
                int_at(c, 5),
            );
            // RDB$NULL_FLAG: 1 = NOT NULL, NULL/0 = nullable
            let null_flag = opt_int_at(c, 6);
            let default_raw = opt_text_at(c, 7).unwrap_or_default();
            let default = decode_blr_default(&default_raw).unwrap_or_default();
            let _domain = text_at(c, 8);
            let mut def = format!("    {name} {type_str}");
            if !default.is_empty() {
                def.push_str(&format!(" DEFAULT {default}"));
            }
            if is_not_null(null_flag) {
                def.push_str(" NOT NULL");
            }
            col_defs.push(def);
        }
        out.push_str(&col_defs.join(",\n"));
        out.push_str("\n);\n\n");

        // Constraints (PK, FK, UNIQUE)
        let con_sql = r#"
            SELECT
                C.RDB$CONSTRAINT_NAME,
                C.RDB$CONSTRAINT_TYPE,
                S.RDB$FIELD_NAME,
                S.RDB$FIELD_POSITION,
                rc.RDB$FIELD_NAME AS REF_COLUMN
            FROM RDB$RELATION_CONSTRAINTS C
            JOIN RDB$INDICES I ON I.RDB$INDEX_NAME = C.RDB$INDEX_NAME
            JOIN RDB$INDEX_SEGMENTS S ON S.RDB$INDEX_NAME = I.RDB$INDEX_NAME
            LEFT JOIN RDB$INDEX_SEGMENTS rc
                ON rc.RDB$INDEX_NAME = I.RDB$FOREIGN_KEY
               AND rc.RDB$FIELD_POSITION = S.RDB$FIELD_POSITION
            WHERE C.RDB$RELATION_NAME = ?
            ORDER BY C.RDB$CONSTRAINT_NAME, S.RDB$FIELD_POSITION
        "#;
        let cons = query_with_param(client, con_sql, &table)?;
        // Group by constraint
        let mut grouped: std::collections::HashMap<String, (String, Vec<String>, Vec<String>)> =
            std::collections::HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for c in &cons.rows {
            let name = text_at(c, 0);
            let ctype = text_at(c, 1);
            let col = text_at(c, 2);
            let ref_col = text_at(c, 4);
            if !grouped.contains_key(&name) {
                order.push(name.clone());
            }
            let entry = grouped
                .entry(name)
                .or_insert_with(|| (ctype, Vec::new(), Vec::new()));
            entry.1.push(col);
            if !ref_col.is_empty() {
                entry.2.push(ref_col);
            }
        }
        for name in order {
            if let Some((ctype, cols, ref_cols)) = grouped.remove(&name) {
                let cols_str = cols
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                match ctype.as_str() {
                    "PRIMARY KEY" => {
                        out.push_str(&format!(
                            "ALTER TABLE {table} ADD CONSTRAINT {name} PRIMARY KEY ({cols_str});\n"
                        ));
                    }
                    "UNIQUE" => {
                        out.push_str(&format!(
                            "ALTER TABLE {table} ADD CONSTRAINT {name} UNIQUE ({cols_str});\n"
                        ));
                    }
                    "FOREIGN KEY" => {
                        // FK needs the referenced table — fetch it
                        let fk_sql = format!(
                            r#"
                            SELECT fc.RDB$RELATION_NAME
                            FROM RDB$RELATION_CONSTRAINTS c
                            JOIN RDB$INDICES ci ON ci.RDB$INDEX_NAME = c.RDB$INDEX_NAME
                            LEFT JOIN RDB$RELATION_CONSTRAINTS fc ON fc.RDB$INDEX_NAME = ci.RDB$FOREIGN_KEY
                            WHERE c.RDB$CONSTRAINT_NAME = ?
                            "#
                        );
                        let fk = query_with_param(client, &fk_sql, &name)?;
                        let ref_table = fk.rows.first().map(|r| text_at(r, 0)).unwrap_or_default();
                        let ref_cols_str = if ref_cols.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", ref_cols.join(", "))
                        };
                        out.push_str(&format!(
                            "ALTER TABLE {table} ADD CONSTRAINT {name} FOREIGN KEY ({cols_str}) REFERENCES {ref_table}{ref_cols_str};\n"
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Indexes (non-constraint)
        let idx_sql = r#"
            SELECT
                I.RDB$INDEX_NAME,
                S.RDB$FIELD_NAME
            FROM RDB$INDICES I
            JOIN RDB$INDEX_SEGMENTS S ON S.RDB$INDEX_NAME = I.RDB$INDEX_NAME
            WHERE I.RDB$RELATION_NAME = ?
              AND I.RDB$INDEX_NAME NOT IN (
                  SELECT RDB$INDEX_NAME FROM RDB$RELATION_CONSTRAINTS WHERE RDB$RELATION_NAME = ?
              )
            ORDER BY I.RDB$INDEX_NAME, S.RDB$FIELD_POSITION
        "#;
        let idxs = query(
            client,
            &idx_sql.replace("?", &format!("'{}'", table.replace('"', "''"))),
        )?;
        let mut grouped_idx: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut idx_order: Vec<String> = Vec::new();
        for i in &idxs.rows {
            let name = text_at(i, 0);
            let col = text_at(i, 1);
            if !grouped_idx.contains_key(&name) {
                idx_order.push(name.clone());
            }
            grouped_idx.entry(name).or_default().push(col);
        }
        for name in idx_order {
            if let Some(cols) = grouped_idx.remove(&name) {
                let cols_str = cols
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("CREATE INDEX {name} ON {table} ({cols_str});\n"));
            }
        }
        out.push('\n');
    }
    Ok(())
}

/// Appends `CREATE VIEW` statements for all user views to the output.
fn extract_views(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT RDB$RELATION_NAME, RDB$VIEW_SOURCE
        FROM RDB$RELATIONS
        WHERE RDB$SYSTEM_FLAG = 0
          AND RDB$VIEW_BLR IS NOT NULL
        ORDER BY RDB$RELATION_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Views\n\n");
            any = true;
        }
        let view_sql = text_at(row, 1);
        out.push_str(&format!("CREATE VIEW {name} AS\n{view_sql};\n\n"));
    }
    Ok(())
}

/// Appends `CREATE SEQUENCE` statements for all user generators to the output.
fn extract_sequences(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT RDB$GENERATOR_NAME
        FROM RDB$GENERATORS
        WHERE RDB$SYSTEM_FLAG = 0
        ORDER BY RDB$GENERATOR_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Sequences\n\n");
            any = true;
        }
        // Get current value via GEN_ID(name, 0) — non-mutating
        let current_value = query_with_param(
            client,
            &format!("SELECT GEN_ID(\"{name}\", 0) FROM RDB$DATABASE"),
            &name,
        )
        .ok()
        .and_then(|r| r.rows.first().and_then(|row| opt_int_at(row, 0)));
        match current_value {
            Some(v) => {
                out.push_str(&format!("CREATE SEQUENCE {name} START WITH {v};\n"));
            }
            None => {
                out.push_str(&format!("CREATE SEQUENCE {name};\n"));
            }
        }
    }
    if any {
        out.push('\n');
    }
    Ok(())
}

/// Appends `CREATE TRIGGER` statements for all user triggers to the output.
fn extract_triggers(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT
            RDB$TRIGGER_NAME,
            RDB$TRIGGER_TYPE,
            RDB$RELATION_NAME,
            RDB$TRIGGER_SOURCE
        FROM RDB$TRIGGERS
        WHERE RDB$SYSTEM_FLAG = 0
        ORDER BY RDB$TRIGGER_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Triggers\n\n");
            any = true;
        }
        let trig_type = int_at(row, 1);
        let table = text_at(row, 2);
        let body = text_at(row, 3);
        let (timing, event) = trigger_type_label(trig_type);
        // RDB$TRIGGER_SOURCE already contains "AS BEGIN ... END" — don't add extra AS/END
        out.push_str(&format!(
            "SET TERM ^ ;\nCREATE TRIGGER {name} FOR {table}\n{timing} {event}\n{body} ^\nSET TERM ; ^\n\n"
        ));
    }
    Ok(())
}

/// Appends `CREATE PROCEDURE` statements for all user procedures to the output.
fn extract_procedures(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT
            RDB$PROCEDURE_NAME,
            RDB$PROCEDURE_SOURCE
        FROM RDB$PROCEDURES
        WHERE RDB$SYSTEM_FLAG = 0
        ORDER BY RDB$PROCEDURE_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Procedures\n\n");
            any = true;
        }
        let source = text_at(row, 1);

        // Fetch parameters (inputs and outputs) from RDB$PROCEDURE_PARAMETERS
        let params_sql = r#"
            SELECT
                RDB$PARAMETER_NAME,
                RDB$PARAMETER_TYPE,
                RDB$FIELD_SOURCE
            FROM RDB$PROCEDURE_PARAMETERS
            WHERE RDB$PROCEDURE_NAME = ?
            ORDER BY RDB$PARAMETER_TYPE, RDB$PARAMETER_NUMBER
        "#;
        let params = query_with_param(client, params_sql, &name)?;
        let mut inputs: Vec<String> = Vec::new();
        let mut outputs: Vec<String> = Vec::new();
        for p in &params.rows {
            let pname = text_at(p, 0);
            let ptype = int_at(p, 1); // 0 = input, 1 = output
            let domain = text_at(p, 2);
            // Resolve domain to full type
            let type_str = resolve_domain_type(client, &domain)?;
            let param_def = format!("{pname} {type_str}");
            if ptype == 0 {
                inputs.push(param_def);
            } else {
                outputs.push(param_def);
            }
        }

        let mut header = format!("CREATE PROCEDURE {name}");
        if !inputs.is_empty() {
            header.push_str(&format!(" ({})", inputs.join(", ")));
        }
        if !outputs.is_empty() {
            header.push_str(&format!(" RETURNS ({})", outputs.join(", ")));
        }

        // RDB$PROCEDURE_SOURCE contains "BEGIN ... END" without "AS" — add AS
        out.push_str(&format!(
            "SET TERM ^ ;\n{header}\nAS\n{source} ^\nSET TERM ; ^\n\n"
        ));
    }
    Ok(())
}

/// Resolves a domain name to its full type string by querying RDB$FIELDS.
fn resolve_domain_type(client: &mut FirebirdClient, domain: &str) -> Result<String, FbError> {
    if domain.is_empty() || domain.starts_with("RDB$") {
        return Ok("VARCHAR(100)".to_string());
    }
    let sql = r#"
        SELECT
            RDB$FIELD_TYPE,
            RDB$FIELD_SUB_TYPE,
            RDB$FIELD_LENGTH,
            RDB$FIELD_PRECISION,
            RDB$FIELD_SCALE
        FROM RDB$FIELDS
        WHERE RDB$FIELD_NAME = ?
    "#;
    let result = query_with_param(client, sql, domain)?;
    if let Some(row) = result.rows.first() {
        Ok(format_column_type(
            int_at(row, 0),
            int_at(row, 1),
            int_at(row, 2),
            int_at(row, 3),
            int_at(row, 4),
        ))
    } else {
        Ok(domain.to_string())
    }
}

/// Decodes a BLR-encoded default value into a SQL literal.
/// Handles common cases: text literals, boolean, current_timestamp.
/// Returns None if the BLR cannot be decoded.
pub fn decode_blr_default(blr_text: &str) -> Option<String> {
    // Check for current_timestamp
    if blr_text.contains("blr_current_timestamp") {
        return Some("CURRENT_TIMESTAMP".to_string());
    }
    // Check for boolean
    if blr_text.contains("blr_bool") {
        // Look for the boolean value (0 or 1) after blr_bool
        let after_bool = blr_text.split("blr_bool").nth(1)?;
        let val = after_bool.trim().chars().next()?;
        return if val == '1' {
            Some("TRUE".to_string())
        } else {
            Some("FALSE".to_string())
        };
    }
    // Check for text literal
    if blr_text.contains("blr_text2") {
        // Format: blr_text2, <len>, <charset>, '<char1>','<char2>',...,blr_eoc
        // Extract everything between blr_text2 and blr_eoc
        let after_text2 = blr_text.split("blr_text2").nth(1)?;
        let before_eoc = after_text2.split("blr_eoc").next()?;
        // The text is encoded as individual characters: 'P','E','N','D','E','N','T','E'
        // Extract all characters between single quotes
        let mut text = String::new();
        let mut in_char = false;
        for c in before_eoc.chars() {
            if c == '\'' {
                in_char = !in_char;
            } else if in_char {
                text.push(c);
            }
        }
        if !text.is_empty() {
            return Some(format!("'{text}'"));
        }
    }
    None
}

/// Appends `CREATE FUNCTION` statements for all user functions to the output.
fn extract_functions(
    client: &mut FirebirdClient,
    out: &mut String,
    filter: &Option<std::collections::HashSet<String>>,
) -> Result<(), FbError> {
    let sql = r#"
        SELECT
            RDB$FUNCTION_NAME,
            RDB$FUNCTION_SOURCE
        FROM RDB$FUNCTIONS
        WHERE RDB$SYSTEM_FLAG = 0
        ORDER BY RDB$FUNCTION_NAME
    "#;
    let result = query(client, sql)?;
    let mut any = false;
    for row in &result.rows {
        let name = text_at(row, 0);
        if !should_extract(filter, &name) {
            continue;
        }
        if !any {
            out.push_str("-- Functions\n\n");
            any = true;
        }
        let source = text_at(row, 1);

        // Fetch parameters from RDB$FUNCTION_ARGUMENTS
        let params_sql = r#"
            SELECT
                fa.RDB$ARGUMENT_NAME,
                fa.RDB$ARGUMENT_POSITION,
                f.RDB$RETURN_ARGUMENT,
                fld.RDB$FIELD_TYPE,
                fld.RDB$FIELD_SUB_TYPE,
                fld.RDB$FIELD_LENGTH,
                fld.RDB$FIELD_PRECISION,
                fld.RDB$FIELD_SCALE
            FROM RDB$FUNCTION_ARGUMENTS fa
            JOIN RDB$FUNCTIONS f
              ON fa.RDB$FUNCTION_NAME = f.RDB$FUNCTION_NAME
             AND COALESCE(fa.RDB$PACKAGE_NAME, '') = COALESCE(f.RDB$PACKAGE_NAME, '')
            JOIN RDB$FIELDS fld
              ON fa.RDB$FIELD_SOURCE = fld.RDB$FIELD_NAME
            WHERE f.RDB$FUNCTION_NAME = ?
            ORDER BY fa.RDB$ARGUMENT_POSITION
        "#;
        let mut inputs: Vec<String> = Vec::new();
        if let Ok(params) = query_with_param(client, params_sql, &name) {
            for p in &params.rows {
                let pname = text_at(p, 0);
                let pos = int_at(p, 1);
                let return_arg = int_at(p, 2);
                let type_str = format_column_type(
                    int_at(p, 3),
                    int_at(p, 4),
                    int_at(p, 5),
                    int_at(p, 6),
                    int_at(p, 7),
                );
                if pos != return_arg {
                    inputs.push(format!("{pname} {type_str}"));
                }
            }
        }

        // Get the return type from RDB$FUNCTION_ARGUMENTS (where position = return_argument)
        let return_sql = r#"
            SELECT
                fld.RDB$FIELD_TYPE,
                fld.RDB$FIELD_SUB_TYPE,
                fld.RDB$FIELD_LENGTH,
                fld.RDB$FIELD_PRECISION,
                fld.RDB$FIELD_SCALE
            FROM RDB$FUNCTION_ARGUMENTS fa
            JOIN RDB$FUNCTIONS f
              ON fa.RDB$FUNCTION_NAME = f.RDB$FUNCTION_NAME
             AND COALESCE(fa.RDB$PACKAGE_NAME, '') = COALESCE(f.RDB$PACKAGE_NAME, '')
            JOIN RDB$FIELDS fld
              ON fa.RDB$FIELD_SOURCE = fld.RDB$FIELD_NAME
            WHERE f.RDB$FUNCTION_NAME = ?
              AND fa.RDB$ARGUMENT_POSITION = f.RDB$RETURN_ARGUMENT
        "#;
        let return_type = if let Ok(result) = query_with_param(client, return_sql, &name) {
            if let Some(row) = result.rows.first() {
                format_column_type(
                    int_at(row, 0),
                    int_at(row, 1),
                    int_at(row, 2),
                    int_at(row, 3),
                    int_at(row, 4),
                )
            } else {
                "INTEGER".to_string()
            }
        } else {
            "INTEGER".to_string()
        };

        let mut header = format!("CREATE FUNCTION {name}");
        if !inputs.is_empty() {
            header.push_str(&format!("({})", inputs.join(", ")));
        }
        header.push_str(&format!(" RETURNS {return_type}"));

        // RDB$FUNCTION_SOURCE contains "BEGIN ... END" without "AS" — add AS
        out.push_str(&format!(
            "SET TERM ^ ;\n{header}\nAS\n{source} ^\nSET TERM ; ^\n\n"
        ));
    }
    Ok(())
}

// --- Describe objects (partial metadata) ---

/// Describes multiple objects by name, returning Markdown with their metadata.
/// Accepts any mix of tables, views, procedures, functions, triggers, sequences, and domains.
pub fn describe_objects(client: &mut FirebirdClient, names: &[String]) -> Result<String, FbError> {
    let mut out = String::new();

    for name in names {
        let upper = name.to_uppercase();

        // Try table/view first
        let rel_sql = format!(
            r#"SELECT RDB$RELATION_NAME, RDB$VIEW_BLR IS NOT NULL
               FROM RDB$RELATIONS
               WHERE RDB$RELATION_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let rel = query(client, &rel_sql)?;
        if !rel.rows.is_empty() {
            let is_view = matches!(rel.rows[0].get(1), Some(Value::Bool(true)));
            out.push_str(&format!(
                "### {} ({})\n\n",
                upper,
                if is_view { "VIEW" } else { "TABLE" }
            ));
            // Reuse schema::describe_table for full details
            if let Ok(schema) = crate::schema::describe_table(client, &upper) {
                out.push_str(&crate::schema::format_schema_markdown(&schema));
            }
            out.push('\n');
            continue;
        }

        // Try procedure
        let proc_sql = format!(
            r#"SELECT RDB$PROCEDURE_NAME, RDB$PROCEDURE_SOURCE
               FROM RDB$PROCEDURES
               WHERE RDB$PROCEDURE_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let proc = query(client, &proc_sql)?;
        if !proc.rows.is_empty() {
            let source = text_at(&proc.rows[0], 1);
            out.push_str(&format!("### {upper} (PROCEDURE)\n\n"));
            // List parameters (RDB$PROCEDURE_PARAMETERS)
            // Columns: RDB$PARAMETER_NAME, RDB$PARAMETER_NUMBER, RDB$PARAMETER_TYPE (0=input, 1=output), RDB$FIELD_SOURCE (domain)
            let params = query(
                client,
                &format!(
                    r#"SELECT RDB$PARAMETER_NAME, RDB$PARAMETER_NUMBER, RDB$PARAMETER_TYPE, RDB$FIELD_SOURCE
                       FROM RDB$PROCEDURE_PARAMETERS
                       WHERE RDB$PROCEDURE_NAME = '{}'
                       ORDER BY RDB$PARAMETER_TYPE, RDB$PARAMETER_NUMBER"#,
                    upper.replace('"', "''")
                ),
            )?;
            // Separate inputs and outputs
            let mut inputs: Vec<(String, i64, String)> = Vec::new();
            let mut outputs: Vec<(String, i64, String)> = Vec::new();
            for p in &params.rows {
                let pname = text_at(p, 0).trim().to_string();
                let pnum = int_at(p, 1);
                let ptype = int_at(p, 2);
                let domain = text_at(p, 3).trim().to_string();
                if ptype == 0 {
                    inputs.push((pname, pnum, domain));
                } else {
                    outputs.push((pname, pnum, domain));
                }
            }
            if !inputs.is_empty() {
                out.push_str("#### Input Parameters\n\n");
                out.push_str("POSITION | NAME | DOMAIN\n");
                out.push_str("--- | --- | ---\n");
                for (pname, pnum, domain) in &inputs {
                    out.push_str(&format!("{pnum} | {pname} | {domain}\n"));
                }
                out.push('\n');
            }
            if !outputs.is_empty() {
                out.push_str("#### Output Parameters\n\n");
                out.push_str("POSITION | NAME | DOMAIN\n");
                out.push_str("--- | --- | ---\n");
                for (pname, pnum, domain) in &outputs {
                    out.push_str(&format!("{pnum} | {pname} | {domain}\n"));
                }
                out.push('\n');
            }
            // Build full signature with parameters
            let mut signature = format!("CREATE PROCEDURE {upper}");
            let all_params: Vec<String> = inputs
                .iter()
                .map(|(n, _, d)| format!("{n} {d}"))
                .chain(outputs.iter().map(|(n, _, d)| format!("{n} {d}")))
                .collect();
            if !all_params.is_empty() {
                signature.push_str(&format!(" ({})", all_params.join(", ")));
            }
            signature.push_str(&format!("\nAS\n{source}"));
            out.push_str(&format!(
                "#### Source\n\n```
{signature}
```\n\n"
            ));
            continue;
        }

        // Try function
        let fn_sql = format!(
            r#"SELECT RDB$FUNCTION_NAME, RDB$FUNCTION_SOURCE
               FROM RDB$FUNCTIONS
               WHERE RDB$FUNCTION_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let fn_res = query(client, &fn_sql)?;
        if !fn_res.rows.is_empty() {
            let source = text_at(&fn_res.rows[0], 1);
            out.push_str(&format!("### {upper} (FUNCTION)\n\n"));
            // Note: RDB$FUNCTION_PARAMETERS does not exist in Firebird 5
            // Parameters are embedded in the source code
            out.push_str(&format!(
                "#### Source\n\n```
CREATE FUNCTION {upper}\nAS\n{source}
```\n\n"
            ));
            continue;
        }

        // Try trigger
        let trig_sql = format!(
            r#"SELECT RDB$TRIGGER_NAME, RDB$TRIGGER_TYPE, RDB$RELATION_NAME, RDB$TRIGGER_SOURCE
               FROM RDB$TRIGGERS
               WHERE RDB$TRIGGER_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let trig = query(client, &trig_sql)?;
        if !trig.rows.is_empty() {
            let row = &trig.rows[0];
            let trig_type = int_at(row, 1);
            let table = text_at(row, 2);
            let source = text_at(row, 3);
            let (timing, event) = trigger_type_label(trig_type);
            out.push_str(&format!(
                "### {upper} (TRIGGER on {table})\n\n{timing} {event}\n\n```
{source}
```\n\n"
            ));
            continue;
        }

        // Try sequence
        let seq_sql = format!(
            r#"SELECT RDB$GENERATOR_NAME FROM RDB$GENERATORS
               WHERE RDB$GENERATOR_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let seq = query(client, &seq_sql)?;
        if !seq.rows.is_empty() {
            out.push_str(&format!("### {upper} (SEQUENCE)\n\n"));
            continue;
        }

        // Try domain
        let dom_sql = format!(
            r#"SELECT RDB$FIELD_NAME, RDB$FIELD_TYPE, RDB$FIELD_SUB_TYPE,
                      RDB$FIELD_LENGTH, RDB$FIELD_PRECISION, RDB$FIELD_SCALE,
                      RDB$DEFAULT_SOURCE, RDB$NULL_FLAG
               FROM RDB$FIELDS
               WHERE RDB$FIELD_NAME = '{}' AND RDB$SYSTEM_FLAG = 0"#,
            upper.replace('"', "''")
        );
        let dom = query(client, &dom_sql)?;
        if !dom.rows.is_empty() {
            let row = &dom.rows[0];
            let type_str = format_column_type(
                int_at(row, 1),
                int_at(row, 2),
                int_at(row, 3),
                int_at(row, 4),
                int_at(row, 5),
            );
            let default = decode_blr_default(&text_at(row, 6)).unwrap_or_default();
            let null_flag = int_at(row, 7);
            // RDB$NULL_FLAG: 1 = NOT NULL, NULL/0 = nullable
            let null_str = if is_not_null(Some(null_flag)) {
                "NOT NULL"
            } else {
                "NULL"
            };
            let default_str = if default.is_empty() {
                String::new()
            } else {
                format!(" DEFAULT {default}")
            };
            out.push_str(&format!(
                "### {upper} (DOMAIN)\n\n{type_str}{default_str} {null_str}\n\n"
            ));
            continue;
        }

        // Not found
        out.push_str(&format!("### {upper}\n\nObject not found.\n\n"));
    }

    Ok(out)
}

// --- Helpers (shared with schema.rs logic) ---

/// Returns the value at `idx` as a trimmed String, or empty string if NULL.
fn text_at(row: &[Value], idx: usize) -> String {
    row.get(idx)
        .map(|v| match v {
            Value::Text(s) => s.trim().to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

/// Returns the value at `idx` as a String, or None if NULL.
fn opt_text_at(row: &[Value], idx: usize) -> Option<String> {
    row.get(idx).and_then(|v| match v {
        Value::Text(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    })
}

/// Returns the value at `idx` as an i64, or None if NULL.
fn opt_int_at(row: &[Value], idx: usize) -> Option<i64> {
    row.get(idx).and_then(|v| match v {
        Value::Int(n) => Some(*n),
        Value::Text(s) => s.parse::<i64>().ok(),
        _ => None,
    })
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
