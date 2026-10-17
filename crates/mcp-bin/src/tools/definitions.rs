//! JSON schemas for all MCP tools (input definitions for `tools/list`).

use serde_json::{Value, json};

/// Builds a single tool definition.
fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

/// Returns the definitions of all tools exposed by the server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "firebird_query",
            "Executes a single SQL statement against the Firebird database. Use for SELECT queries, INSERT, UPDATE, DELETE, CREATE TABLE, ALTER TABLE, DROP, etc. SELECT results are bounded to 100 rows by default (hard cap: 1000); use max_rows and skip to paginate. For DML/DDL the execution plan is appended to the result.",
            json!({
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The raw SQL statement to execute. Single statement only — no SET TERM, no multiple statements." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." },
                    "max_rows": { "type": "string", "description": "Maximum rows to return for SELECT (default: 100, hard cap: 1000). Ignored for DML/DDL." },
                    "skip": { "type": "string", "description": "Number of rows to skip (for pagination). Use the value suggested in the truncation hint. Default: 0." }
                },
                "required": ["sql"]
            }),
        ),
        tool(
            "firebird_describe_table",
            "Shows the full schema of a table or view: columns (with type, size, nullability, default), indexes, constraints, triggers, and comments.",
            json!({
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "Table or view name (without quotes, case-insensitive)." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table_name"]
            }),
        ),
        tool(
            "firebird_find_column",
            "Finds columns by name pattern across all tables and views. Supports % wildcards (e.g., 'EMAIL%' or '%NOME%'). Returns table name, column name, type, nullability, and whether it's in a view.",
            json!({
                "type": "object",
                "properties": {
                    "column": { "type": "string", "description": "Column name pattern (supports % wildcards). E.g., 'EMAIL', '%NOME%', 'ID'" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["column"]
            }),
        ),
        tool(
            "firebird_domains",
            "Analyzes user-defined domains: lists each domain with its type, nullability, default, and usage count (how many columns use it), shows the most common column type signatures, and suggests domains that could be created for common types that lack one.",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_check_duplicates",
            "Checks for duplicate values in one or more columns. Use this BEFORE creating a unique index or constraint to verify data integrity. Supports composite keys (multiple columns). Returns sample duplicate rows if found.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to check." },
                    "columns": { "type": "array", "items": { "type": "string" }, "description": "Column name(s) to check for duplicates. For composite keys, provide all columns." },
                    "limit": { "type": "string", "description": "Maximum number of duplicate rows to return (default: 10)." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table", "columns"]
            }),
        ),
        tool(
            "firebird_get_databases",
            "Lists all databases configured in config.json.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "firebird_create_database",
            "Creates a new Firebird database file (.fdb). The path must be within one of the allowed_paths directories defined in config.json. Page size is configurable (default: 8192); valid values are 1024, 2048, 4096, 8192, 16384 or 32768.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Full path for the new .fdb file. Must be inside an allowed path from config.json." },
                    "page_size": { "type": "string", "description": "Page size in bytes. Must be a power of two between 1024 and 32768 (1K, 2K, 4K, 8K, 16K or 32K). Default: 8192." }
                },
                "required": ["path"]
            }),
        ),
        tool(
            "firebird_run_script",
            "Executes a SQL script (multiple statements) in a single transaction. Handles SET TERM blocks, BEGIN/END nesting, strings, and comments correctly. If any statement fails, the entire transaction is rolled back. The isolation level and lock-wait timeout are configurable (defaults: read-committed, 10s).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). This argument is required to avoid ambiguity." },
                    "script": { "type": "string", "description": "The SQL script to execute. Supports SET TERM ^ blocks, EXECUTE BLOCK, CREATE PROCEDURE, etc." },
                    "path": { "type": "string", "description": "Path to a .sql file to execute. Alternative to providing the script inline." },
                    "isolation": { "type": "string", "enum": ["read-committed", "snapshot", "consistency", "readonly"], "description": "Transaction isolation level. Default: read-committed. 'snapshot' = consistent read-rewrite; 'consistency' = table locking (strictest); 'readonly' = audit scripts that must not write." },
                    "lock_wait": { "type": "string", "description": "Maximum seconds to wait for a lock before failing. Default: 10. Use '0' or omit for indefinite wait." }
                },
                "required": ["database"]
            }),
        ),
        tool(
            "firebird_create_trigger",
            "Creates a new trigger in the database. The body should contain only the SQL statements (without BEGIN/END or SET TERM).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json)." },
                    "trigger_name": { "type": "string", "description": "Name for the new trigger." },
                    "table_name": { "type": "string", "description": "Table the trigger will be associated with." },
                    "trigger_type": { "type": "string", "description": "Trigger event type: INSERT, UPDATE, or DELETE." },
                    "timing": { "type": "string", "description": "Trigger timing: BEFORE or AFTER." },
                    "body": { "type": "string", "description": "The trigger body (SQL statements only). Do NOT include SET TERM, BEGIN, or END." }
                },
                "required": ["database", "trigger_name", "table_name", "trigger_type", "timing", "body"]
            }),
        ),
        tool(
            "firebird_insert_batch",
            "Inserts multiple rows into a table in a single transaction using a prepared statement. All rows must have the same number of values as columns. The entire batch is atomic — if any row fails, all are rolled back.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to insert into." },
                    "columns": { "type": "string", "description": "Column names as a JSON array. E.g. [\"NAME\", \"EMAIL\", \"AGE\"]" },
                    "rows": { "type": "string", "description": "Row data as a JSON array of arrays. E.g. [[\"Alice\", \"a@x.com\", 30], [\"Bob\", \"b@x.com\", 25]]" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table", "columns", "rows"]
            }),
        ),
        tool(
            "firebird_count",
            "Returns the number of rows in a table, optionally filtered by a WHERE clause. Use this instead of firebird_query with SELECT COUNT(*) for a cleaner, more token-efficient result.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to count rows from." },
                    "where": { "type": "string", "description": "Optional WHERE clause (without the WHERE keyword). E.g. \"STATUS = 'ACTIVE'\"" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table"]
            }),
        ),
        tool(
            "firebird_sample",
            "Returns a sample of rows from a table (default: 10 rows), optionally filtered by a WHERE clause. Use this to quickly inspect table data without writing a full SELECT query.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to sample from." },
                    "where": { "type": "string", "description": "Optional WHERE clause (without the WHERE keyword)." },
                    "limit": { "type": "string", "description": "Maximum number of rows to return. Defaults to 10." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table"]
            }),
        ),
        tool(
            "firebird_execute_immediate",
            "Executes a SQL statement that may contain SET TERM blocks (e.g. CREATE PROCEDURE, CREATE GENERATOR, CREATE FUNCTION). For regular DML/DDL (CREATE TABLE, ALTER TABLE, etc.), use firebird_query instead.",
            json!({
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The SQL statement to execute. May include SET TERM blocks." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["sql"]
            }),
        ),
        tool(
            "firebird_describe_database",
            "Provides a complete overview of the database structure. Lists all tables with their columns, types, and constraints, and generates a Mermaid ER diagram showing all relationships.",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." },
                    "include_counts": { "type": "boolean", "description": "If true, includes row counts for each table. May be slow for large tables. Defaults to false." },
                    "include_describes": { "type": "boolean", "description": "If true, includes column details for each table. Defaults to true." }
                }
            }),
        ),
        tool(
            "firebird_list",
            "Lists database objects by type. Supported types: TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, SEQUENCES.",
            json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "The object type: TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, or SEQUENCES" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["type"]
            }),
        ),
        tool(
            "firebird_metadata_extract",
            "Extracts database metadata as a DDL script (replaces isql -x). By default extracts everything; optionally pass 'objects' to extract only specific objects (tables, views, procedures, functions, triggers, sequences, domains).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." },
                    "objects": { "type": "array", "items": { "type": "string" }, "description": "Optional list of object names to extract. If omitted, extracts everything." }
                }
            }),
        ),
        tool(
            "firebird_describe",
            "Describes multiple database objects by name, returning their metadata in Markdown. Accepts any mix of tables, views, procedures, functions, triggers, sequences, and domains. For tables/views, returns full schema (columns, indexes, constraints, triggers, comments). For procedures/functions/triggers, returns the source code.",
            json!({
                "type": "object",
                "properties": {
                    "objects": { "type": "array", "items": { "type": "string" }, "description": "List of object names to describe (case-insensitive)." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["objects"]
            }),
        ),
        tool(
            "firebird_alter_column_type",
            "Changes the data type of a column to use a domain. Uses the Firebird syntax: ALTER TABLE table_name ALTER column_name TYPE domain_name. If the column is part of a PK, FK, or has a UNIQUE constraint, Firebird will block the alteration.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "The table name" },
                    "column": { "type": "string", "description": "The column name" },
                    "domain": { "type": "string", "description": "The domain name to apply" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["table", "column", "domain"]
            }),
        ),
        tool(
            "firebird_drop",
            "Drops a database object by type and name. Supported types: TABLE, VIEW, DOMAIN, TRIGGER, PROCEDURE, FUNCTION, INDEX, SEQUENCE.",
            json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "The object type: TABLE, VIEW, DOMAIN, TRIGGER, PROCEDURE, FUNCTION, INDEX, or SEQUENCE" },
                    "name": { "type": "string", "description": "The name of the object to drop" },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["type", "name"]
            }),
        ),
        tool(
            "firebird_explain_plan",
            "Returns the execution plan for a SQL statement. IMPORTANT: the statement IS executed (inside a transaction that is always rolled back, so no data changes persist) in order to obtain the plan — for a very expensive statement this still costs the full execution time. Use this to inspect the plan of a query BEFORE running it for real (e.g. a nested DELETE that could take minutes).",
            json!({
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The SQL statement to get the execution plan for. It will be executed (and rolled back) to obtain the plan." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["sql"]
            }),
        ),
        tool(
            "firebird_create_procedure",
            "Creates a new stored procedure in the database. The body should contain only the PSQL statements (without BEGIN/END or SET TERM).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json)." },
                    "procedure_name": { "type": "string", "description": "Name for the new procedure." },
                    "params": { "type": "string", "description": "Optional parameter list, e.g. \"ID INTEGER, NAME VARCHAR(100)\". Leave empty for no parameters." },
                    "body": { "type": "string", "description": "The procedure body (PSQL statements only). Do NOT include SET TERM, BEGIN, or END." }
                },
                "required": ["database", "procedure_name", "body"]
            }),
        ),
        tool(
            "firebird_create_function",
            "Creates a new user-defined function in the database. The body should contain only the PSQL statements (without BEGIN/END or SET TERM).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json)." },
                    "function_name": { "type": "string", "description": "Name for the new function." },
                    "params": { "type": "string", "description": "Optional parameter list, e.g. \"A INTEGER, B INTEGER\". Leave empty for no parameters." },
                    "return_type": { "type": "string", "description": "The return type, e.g. INTEGER, VARCHAR(100), TIMESTAMP." },
                    "body": { "type": "string", "description": "The function body (PSQL statements only). Do NOT include SET TERM, BEGIN, or END." }
                },
                "required": ["database", "function_name", "return_type", "body"]
            }),
        ),
        tool(
            "firebird_create_sequence",
            "Creates a new sequence (generator) in the database.",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json)." },
                    "sequence_name": { "type": "string", "description": "Name for the new sequence." }
                },
                "required": ["database", "sequence_name"]
            }),
        ),
        tool(
            "firebird_generate_dev",
            "Generates development code (entity/struct/interface) for one or more tables. Supports Java (JPA + Lombok), Go (sqlx), Rust (sqlx), TypeScript, Protobuf, OpenAPI, and Lazarus (.lfm forms). Use include_fks=true to generate named FK relations (Java @ManyToOne) or FK comments (Go/Rust/TS). For Lazarus, generates basic forms with TFBDataSet, TDataSource, and DB components (TDBEdit, TDBMemo, TDBDateTimePicker, TDBCheckBox).",
            json!({
                "type": "object",
                "properties": {
                    "tables": { "type": "array", "items": { "type": "string" }, "description": "Table names to generate code for (max 20)." },
                    "language": { "type": "string", "enum": ["java", "go", "rust", "typescript", "protobuf", "openapi", "lazarus"], "description": "Target language. Use 'lazarus' to generate .lfm form files." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." },
                    "include_fks": { "type": "boolean", "description": "If true, generates named FK relations (Java @ManyToOne) or FK comments (Go/Rust/TS). Defaults to false." }
                },
                "required": ["tables", "language"]
            }),
        ),
        tool(
            "firebird_generate_view_dto",
            "Generates DTOs/records for one or more views. Uses resolved metadata from the view (executes SELECT FIRST 1 * FROM view) to get accurate column types. Generated code is read-only (no entity annotations, no FKs). Supports Java (Lombok @Data), Go (sqlx tags), Rust (FromRow), TypeScript (interface), Protobuf, OpenAPI, and Lazarus (.lfm forms).",
            json!({
                "type": "object",
                "properties": {
                    "views": { "type": "array", "items": { "type": "string" }, "description": "View names to generate DTOs for (max 20)." },
                    "language": { "type": "string", "enum": ["java", "go", "rust", "typescript", "protobuf", "openapi", "lazarus"], "description": "Target language. Use 'lazarus' to generate .lfm form files." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                },
                "required": ["views", "language"]
            }),
        ),
        tool(
            "firebird_crypt_state",
            "Returns the at-rest encryption state of the database (MON$DATABASE.MON$CRYPT_STATE, Firebird 3.0+). Values: unencrypted, encrypted, decrypt in progress, encrypt in progress. Also reports the engine version.",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_database_stats",
            "Returns a snapshot of database statistics: MON$DATABASE (size, page size, ODS, dialect, owner, creation date, read-only/forced-writes flags, backup/shutdown state), transaction counters (OIT/OAT/OST/next/sweep), cumulative I/O (reads/writes/fetches/marks), live attachments (MON$ATTACHMENTS), and active statements (MON$STATEMENTS).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_query_history",
            "Returns the query history: all active statements from MON$STATEMENTS, joined with MON$ATTACHMENTS for user and remote address. Useful for seeing what queries are currently running or idle on the database. Filter by state (0=idle, 1=active, 2=stalled) and/or user name. Results are bounded to 100 rows by default (hard cap: 1000).",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." },
                    "state": { "type": "string", "description": "Filter by statement state: 0=idle, 1=active, 2=stalled. Omit to include all states." },
                    "user": { "type": "string", "description": "Filter by user name (case-insensitive). Omit to include all users." },
                    "limit": { "type": "string", "description": "Maximum number of rows to return (default: 100, hard cap: 1000)." }
                }
            }),
        ),
        tool(
            "firebird_table_stats",
            "Returns per-table statistics: row count for each table (or a single table). Useful for identifying large tables and estimating data distribution. Note: COUNT(*) on very large tables can be slow.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to get stats for. Omit to get stats for all tables." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_users",
            "Lists all users defined in the database (RDB$USERS). Shows user name, type, active status, and creation info.",
            json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_grants",
            "Shows GRANT privileges for a specific table or all tables (RDB$RELATION_PRIVILEGES, Firebird 3+). Lists grantee, privilege type (SELECT, INSERT, UPDATE, DELETE, REFERENCES, ALTER), and whether the grant is grantable.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to show grants for. Omit to show grants for all tables." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_blob_info",
            "Lists all BLOB columns in the database (or a specific table): table, column, blob subtype (TEXT/BINARY), and field length. Useful for identifying BLOB-heavy tables.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to list BLOB columns for. Omit to list BLOB columns in all tables." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
        tool(
            "firebird_index_usage",
            "Shows all indexes in the database (or a specific table): index name, table, uniqueness (0=non-unique, 1=unique, 2=PK), index type, columns (in order), and column count. Useful for identifying redundant or missing indexes.",
            json!({
                "type": "object",
                "properties": {
                    "table": { "type": "string", "description": "Table name to show indexes for. Omit to show indexes for all tables." },
                    "database": { "type": "string", "description": "Database name (as defined in config.json). Defaults to the default database." }
                }
            }),
        ),
    ]
}
