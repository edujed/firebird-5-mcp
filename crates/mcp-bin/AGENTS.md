# mcp-bin — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-bin`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this crate is:** the MCP stdio server binary (`firebird-5-mcp`). It speaks
> JSON-RPC 2.0 (line-delimited) over stdin/stdout and exposes Firebird tools. All
> heavy lifting (client, query, schema, codegen, storage, config) lives in the
> sibling crate `firebird-5-mcp-core` (`crates/mcp-core`) — this crate is a thin
> layer of argument parsing + Markdown rendering + dispatch.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 13 | Package `firebird-5-mcp`; deps: `firebird-5-mcp-core`, `serde_json`, `chrono` |
| `src/main.rs` | 166 | Entry point, MCP protocol loop |
| `src/tools/mod.rs` | 119 | `ToolResult`, `dispatch` (tool name → handler) |
| `src/tools/common.rs` | 199 | Shared helpers (arg parsing, client, Markdown, file save) |
| `src/tools/definitions.rs` | 425 | JSON input schemas for `tools/list` |
| `src/tools/query_tools.rs` | 221 | SQL execution tools |
| `src/tools/schema_tools.rs` | 926 | Schema inspection tools (largest file) |
| `src/tools/stats_tools.rs` | 233 | MON$ statistics tools |
| `src/tools/introspection_tools.rs` | 270 | RDB$ introspection tools |
| `src/tools/generator_tools.rs` | 173 | `firebird_generate_dev` handler |
| `src/tools/view_dto_tools.rs` | 78 | `firebird_generate_view_dto` handler |
| `src/tools/ddl_tools.rs` | 229 | DDL create/alter/drop tools |

## `src/main.rs`

| Function | Lines | Summary |
|---|---|---|
| `build_time()` | 30–38 | Compile-time build timestamp via `OnceLock` + `chrono` |
| `main()` | 40–166 | Loads `Config`, inits file logging, then loops over stdin lines: parses JSON-RPC, handles `initialize` / `ping` / `tools/list` / `tools/call` (via `dispatch`), writes line-delimited responses. Notifications (no `id`) are acked silently. |

Key statics: `CONFIG: RwLock<Option<Config>>` (L22), `SERVER_NAME`/`SERVER_VERSION`/`PROTOCOL_VERSION` (L24–26).

## `src/tools/mod.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct ToolResult` | 33–36 | `{ text: String, structured: Option<Value> }` — text always present, structured optional |
| `ToolResult::text` / `with_structured` | 39–51 | Constructors |
| `wrap_text()` | 56–58 | Adapts `Result<String,String>` handlers to `Result<ToolResult,String>` |
| `dispatch()` | 61–68 | Logs the call, delegates to `dispatch_inner` |
| `dispatch_inner()` | 70–119 | `match` on tool name → handler. **This is the tool registry.** Groups: query (73–79), schema (82–90), stats (93–95), introspection (98–102), generators (105–106), DDL (109–115) |

## `src/tools/common.rs`

| Function | Lines | Summary |
|---|---|---|
| `arg_str` | 12–16 | Extract string arg from JSON |
| `arg_bool` | 19–21 | Extract bool arg |
| `arg_u32` | 26–32 | Extract non-negative int; accepts JSON number **or** numeric string (LLMs mix types) |
| `open_client` | 35–46 | Opens `FirebirdClient` for `database` arg or the default db |
| `result_to_text` | 49–90 | Renders `query::QueryResult` as a Markdown table |
| `plural_suffix` | 93–95 | `""` if 1 else `"s"` |
| `cell_text` / `cell_int` / `cell_text_or` | 98–119 | Safe cell accessors (defaults `-`, `0`, or given default) |
| `storage_dir` | 125–137 | Output dir priority: `save_to` arg > `config.storage.dir` > `~/.firebird-mcp/output/` |
| `save_or_inline` | 146–199 | Writes generated files to disk, builds previews (first 10 + last 5 lines) and structured content |

## `src/tools/definitions.rs`

| Function | Lines | Summary |
|---|---|---|
| `tool()` | 6–12 | Builds one `{name, description, inputSchema}` object |
| `tool_definitions()` | 15–425 | Returns all tool JSON schemas (one `tool(...)` per MCP tool). **Source of truth for tool descriptions/args** — keep in sync with `dispatch_inner` |

## `src/tools/query_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_query` | 13–57 | Executes one SQL statement. SELECT → bounded rows (default 100, cap 1000) + pagination hint; DML/DDL → appends execution plan |
| `handle_count` | 60–84 | `COUNT(*)` on a table, optional WHERE |
| `handle_sample` | 87–105 | First N rows (default 10) of a table |
| `handle_insert_batch` | 108–146 | Bulk INSERT; `columns`/`rows` arrive as JSON-encoded strings |
| `handle_run_script` | 149–195 | Multi-statement script in one transaction; `path` arg restricted to `allowed_paths`; configurable isolation + lock wait |
| `handle_execute_immediate` | 198–208 | Executes SQL containing `SET TERM` blocks (procedures, generators, functions) |
| `handle_explain_plan` | 211–221 | Returns execution plan; statement runs inside a rolled-back transaction |

## `src/tools/schema_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `format_schema` | 10–96 | Renders `schema::TableSchema` as Markdown (columns, indexes, constraints, triggers, comments) |
| `handle_describe_table` | 98–110 | Full schema of one table/view |
| `handle_get_databases` | 112–142 | Reloads config from disk, lists configured databases + build time |
| `handle_describe_database` | 144–322 | Full DB overview: MON$DATABASE health, per-table columns (PK/NOT NULL/comments), optional row counts, Mermaid ER diagram of FKs |
| `handle_list` | 324–559 | Lists objects by type: TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, SEQUENCES (one branch each) |
| `handle_metadata_extract` | 561–575 | DDL extraction (isql `-x` replacement); optional `objects` filter |
| `handle_describe` | 577–594 | Describes multiple objects by name (tables, views, procs, functions, triggers, sequences, domains) |
| `handle_check_duplicates` | 597–722 | Finds duplicate values in 1+ columns (GROUP BY/HAVING), then shows full rows of the first duplicate combo |
| `handle_domains` | 726–865 | Domain analysis: usage counts, most common type signatures, suggestions for missing domains |
| `handle_find_column` | 868–926 | Finds columns by name pattern (`%` wildcards) across all tables/views |

## `src/tools/stats_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_crypt_state` | 9–22 | At-rest encryption state + engine version |
| `handle_database_stats` | 26–129 | MON$DATABASE snapshot: size, ODS, dialect, transactions, I/O, attachments, active statements |
| `backup_state_label` / `shutdown_mode_label` / `attachment_state_label` / `statement_state_label` | 131–166 | Integer → label mappers |
| `truncate` | 168–174 | Truncates strings with `…` |
| `handle_query_history` | 178–233 | Active statements from MON$STATEMENTS joined with MON$ATTACHMENTS; filter by state/user, limit ≤ 1000 |

## `src/tools/introspection_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_table_stats` | 20–68 | Row counts: single table via `COUNT(*)`, all tables via `EXECUTE BLOCK` + dynamic `EXECUTE STATEMENT` |
| `handle_users` | 75–94 | DB owners + users with grants (RDB$DB_CREATORS FULL JOIN RDB$USER_PRIVILEGES) |
| `handle_grants` | 100–141 | GRANT privileges per table or all tables |
| `handle_blob_info` | 147–210 | BLOB columns: subtype (TEXT/BINARY) and length |
| `handle_index_usage` | 217–269 | Indexes with columns (LIST() window fn), uniqueness, type |

## `src/tools/generator_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_generate_dev` | 14–173 | Generates dev code for up to 20 tables (java, go, rust, typescript, protobuf, openapi, graphql, lazarus, svelte). Special paths: Lazarus → `.lfm` forms; Svelte → Page + Form components. Rest goes through `generator::generate` + `save_or_inline` |

## `src/tools/view_dto_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_generate_view_dto` | 12–78 | Generates read-only DTOs for up to 20 views (same languages minus lazarus/svelte); uses resolved view metadata |

## `src/tools/ddl_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_create_database` | 10–87 | Creates `.fdb` inside `allowed_paths`; optional `register` writes to config.json; returns structured content |
| `DEFAULT_PAGE_SIZE` / `is_valid_page_size` | 90–95 | 8192 default; only 1K–32K powers of two |
| `handle_create_trigger` | 97–128 | Creates a trigger (name, table, type, timing, body) |
| `handle_create_procedure` | 130–147 | Creates a stored procedure |
| `handle_create_function` | 149–168 | Creates a UDF |
| `handle_create_sequence` | 170–183 | Creates a sequence/generator |
| `handle_alter_column_type` | 185–196 | Changes a column's type to a domain |
| `handle_drop` | 198–229 | Drops TABLE/VIEW/DOMAIN/TRIGGER/PROCEDURE/FUNCTION/INDEX/SEQUENCE (whitelist-validated) |

## Conventions

- Handlers return `Result<String, String>` (Markdown text, error string) or
  `Result<ToolResult, String>` when they support structured output (generators,
  `create_database`).
- Table/column names are uppercased and validated with `is_valid_identifier`
  before being interpolated into SQL; user-supplied values are bound as
  parameters where possible (`query_with_param`).
- File output (generators) resolves dir via `storage_dir` and writes via
  `save_or_inline` (core `storage` module).
- Logging goes through `mlog!` (core) to `{base}/logs/`.
