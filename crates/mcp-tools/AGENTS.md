# mcp-tools — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-tools`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this crate is:** the shared MCP tool handlers (transport-agnostic). It
> contains the **business logic** of the Firebird MCP tools: the handlers that
> parse JSON arguments, call into `firebird-5-mcp-core`, and return a
> Markdown-formatted `ToolResult`. It has **no dependency on any transport
> protocol** (stdio, HTTP, SSE) — that is the job of the consuming crates
> (`mcp-stdio-bin` for stdio, and the future `mcp-http-bin` for HTTP/SSE).

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 15 | Package `firebird-5-mcp-tools`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-types`, `firebird-5-mcp-migration`, `serde_json`, `chrono` |
| `src/lib.rs` | ~200 | `ToolResult`, `dispatch()` (with universal POST_EVENT), `build_time()`, re-exports |
| `src/common.rs` | 322 | Shared helpers (arg parsing, client, Markdown, file save, `download_url` builder) |
| `src/definitions.rs` | ~510 | JSON input schemas for `tools/list` |
| `src/events.rs` | 353 | Universal POST_EVENT support — `emit_tool_event()` (called from `dispatch()`), `valid_event_name()`, `build_event()`, `resolve_database()`, `append_event_line()` |
| `src/protocol.rs` | ~150 | `ProtocolCore` — transport-agnostic JSON-RPC routing (extracted from `mcp-stdio-bin`). Holds the shared `RwLock<Option<Config>>` and routes `initialize`/`ping`/`tools/list`/`tools/call`. Thread-safe, shared between stdio and SSE transports |
| `src/query_tools.rs` | 221 | SQL execution tools |
| `src/schema_tools.rs` | 1258 | Schema inspection tools (largest file) |
| `src/stats_tools.rs` | 233 | MON$ statistics tools |
| `src/introspection_tools.rs` | 270 | RDB$ introspection tools |
| `src/generator_tools.rs` | 201 | `firebird_generate_dev` handler (Python emits `__init__.py`, JSON Schema emits `.json`) |
| `src/view_dto_tools.rs` | 90 | `firebird_generate_view_dto` handler |
| `src/ddl_tools.rs` | 229 | DDL create/alter/drop tools |
| `src/migration_tools.rs` | 346 | Export/import/describe handlers (format-agnostic: Parquet, IPC, CSV, DBF, JSONL + compression + codepage). **Pure handlers** — no event code (POST_EVENT is universal in `dispatch()`) |
| `src/quality_tools.rs` | 687 | `firebird_data_quality` handler — rule-based data quality checks (nulls, empty_strings, outliers, date_range) with Markdown/JSON output |
| `src/file_tools.rs` | 337 | `firebird_list_files` handler — lists generated files in the storage directory with optional filters (database, format, max_age_hours). Returns Markdown table + structured content |

## `src/lib.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct ToolResult` | 77–80 | `{ text: String, structured: Option<Value> }` — text always present, structured optional |
| `ToolResult::text` / `with_structured` | 83–95 | Constructors |
| `wrap_text()` | 100–102 | Adapts `Result<String,String>` handlers to `Result<ToolResult,String>` |
| `dispatch()` | 111–134 | Logs the call, times the handler, delegates to `dispatch_inner`, then calls `emit_tool_event()` (universal POST_EVENT). Takes `files_base_url: Option<&str>` (SSE base URL, `None` in stdio) and threads it to file-generating handlers |
| `dispatch_inner()` | 136–194 | `match` on tool name → handler. **This is the tool registry.** Groups: query, schema, stats, introspection, generators, DDL, migration, data quality. File-generating handlers (generators, `metadata_extract`, export/import/describe_file, `list_files`) receive `files_base_url` |
| `build_time()` | 37–41 | Compile-time build timestamp via `OnceLock` + `chrono` (single source of truth, used by both binaries) |

## `src/events.rs`

| Function | Lines | Summary |
|---|---|---|
| `events_file()` | 26–28 | Path to `{base}/events.jsonl` |
| `valid_event_name()` | 32–36 | Returns `Some(trimmed)` if the event name is present and non-empty after trim, `None` otherwise |
| `build_event()` | 43–71 | Builds the structured event payload: `{event, tool, success, timestamp, database?, duration_ms?, detail?, error?}` |
| `resolve_database()` | 77–87 | Resolves the database for a tool call: `database` arg → `cfg.default_database()` → validates via `cfg.server.find_database()`. Returns `None` when no valid db (e.g. `firebird_get_databases` has no `database` arg and no default) |
| `emit_tool_event()` | 98–162 | **Universal entry point** called from `dispatch()`. (1) Appends a JSON line to `events.jsonl` (always), (2) if a valid database is resolved, opens a **fresh** connection and executes `POST_EVENT '<name>'` in its own transaction. Never fails the tool call — all errors logged via `mlog!` and dropped |
| `open_client_for_event()` | 165–171 | Opens a fresh `FirebirdClient` for the given database (used by `emit_tool_event`) |
| `append_event_line()` | 175–188 | Appends a JSON line to the events file, creating the base dir if needed |

## `src/common.rs`

| Function | Lines | Summary |
|---|---|---|
| `arg_str` | 12–16 | Extract string arg from JSON |
| `arg_bool` | 19–21 | Extract bool arg |
| `arg_u32` | 26–32 | Extract non-negative int; accepts JSON number **or** numeric string (LLMs mix types) |
| `open_client` | 35–46 | Opens `FirebirdClient` for `database` arg or the default db |
| `result_to_text` | 49–90 | Renders `query::QueryResult` as a Markdown table |
| `plural_suffix` | 93–95 | `""` if 1 else `"s"` |
| `cell_text` / `cell_int` / `cell_text_or` | 98–119 | Safe cell accessors (defaults `-`, `0`, or given default) |
| `storage_dir` | 125–137 | Output dir priority: `outputDirPath` arg > `config.storage.dir` > `~/.firebird-mcp/output/` |
| `percent_encode_segment` | 144–158 | Dependency-free percent-encoder for a single path segment (keeps unreserved chars, `%XX`-encodes the rest) |
| `file_download_url` | 166–188 | Builds `{base}/files?path=<encoded rel>` for a file inside the storage dir. Returns `None` when `base_url` is `None` (stdio) or the file is outside the storage base. 5 unit tests |
| `save_or_inline` | 195–262 | Writes generated files to disk, builds previews (first 10 + last 5 lines) and structured content. Takes `files_base_url: Option<&str>`; adds `download_url` to each file entry when available |

## `src/definitions.rs`

| Function | Lines | Summary |
|---|---|---|
| `tool()` | 6–12 | Builds one `{name, description, inputSchema}` object |
| `tool_definitions()` | 15–495 | Returns all tool JSON schemas (one `tool(...)` per MCP tool). **Source of truth for tool descriptions/args** — keep in sync with `dispatch_inner` |

## `src/query_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_query` | 13–57 | Executes one SQL statement. SELECT → bounded rows (default 100, cap 1000) + pagination hint; DML/DDL → appends execution plan |
| `handle_count` | 60–84 | `COUNT(*)` on a table, optional WHERE |
| `handle_sample` | 87–105 | First N rows (default 10) of a table |
| `handle_insert_batch` | 108–146 | Bulk INSERT; `columns`/`rows` arrive as JSON-encoded strings |
| `handle_run_script` | 149–195 | Multi-statement script in one transaction; `path` arg restricted to `allowed_paths`; configurable isolation + lock wait |
| `handle_execute_immediate` | 198–208 | Executes SQL containing `SET TERM` blocks (procedures, generators, functions) |
| `handle_explain_plan` | 211–221 | Returns execution plan; statement runs inside a rolled-back transaction |

## `src/schema_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `format_schema` | 14–20 | Thin wrapper over `schema::format_schema_markdown` (mcp-core) — adds the `### Table: <name>` header |
| `handle_describe_table` | 22–32 | Full schema of one table/view |
| `handle_get_databases` | 36–46 | Reloads config from disk, lists configured databases + build time |
| `handle_describe_database` | 325–483 | Full DB overview: MON$DATABASE health, per-table columns (PK/NOT NULL/comments), optional row counts, Mermaid ER diagram of FKs (shared `build_er_diagram`). Args: `database`, `include_counts`, `include_describes`, `output_mode` (`inline` default / `download`), `output_format` (`markdown` default / `json`), `compression` (zstd/gzip/lz4/none, when saving), `outputDirPath` (optional; `download` without it uses the default storage dir). JSON shape: `{database, health, tables[], er_diagram}`. File naming: `<db>_describe.md` / `<db>_describe.json` (+compression suffix). Returns `ToolResult` (not `wrap_text`) so `files[]`/`download_url` are emitted in SSE mode |
| `collect_db_health` | 70–140 | Reads MON$DATABASE + engine version into a health struct (shared by `describe_database` and `database_stats` rendering) |
| `collect_table_schema` | 142–203 | Fetches columns/PK/NOT NULL/comments for one table (used by `describe_database`) |
| `render_describe_database_markdown` | 205–323 | Renders the full DB overview as Markdown (health, per-table columns, optional row counts, ER diagram) |
| `handle_list` | 485–717 | Lists objects by type: TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, SEQUENCES (one branch each) |
| `handle_metadata_extract` | 719–770 | DDL extraction (isql `-x` replacement); optional `objects` filter; `outputDirPath` saves to file; `compression` param (zstd/gzip/lz4/none) |
| `handle_describe` | 772–789 | Describes multiple objects by name (tables, views, procs, functions, triggers, sequences, domains) |
| `build_er_diagram` | 796–840 | Builds a Mermaid `erDiagram` string (tables + FK relationships) — shared by `handle_er_diagram` and `handle_describe_database` |
| `handle_er_diagram` | 842–918 | Generates a Mermaid `erDiagram` (tables + FK relationships). Args: `database`, optional `tables` (filter), `include_columns` (bool, default false), `outputDirPath` (save to `.mmd`), `compression` (zstd/gzip/lz4/none). Returns the diagram as a Markdown code block; when `outputDirPath` is set, saves via `save_or_inline` (with `download_url` in SSE mode) |
| `handle_check_duplicates` | 921–1046 | Finds duplicate values in 1+ columns (GROUP BY/HAVING), then shows full rows of the first duplicate combo |
| `handle_domains` | 1050–1193 | Domain analysis: usage counts, most common type signatures, suggestions for missing domains |
| `handle_find_column` | 1196–1258 | Finds columns by name pattern (`%` wildcards) across all tables/views |

## `src/stats_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_crypt_state` | 9–22 | At-rest encryption state + engine version |
| `handle_database_stats` | 26–129 | MON$DATABASE snapshot: size, ODS, dialect, transactions, I/O, attachments, active statements |
| `backup_state_label` / `shutdown_mode_label` / `attachment_state_label` / `statement_state_label` | 131–166 | Integer → label mappers |
| `truncate` | 168–174 | Truncates strings with `…` |
| `handle_query_history` | 178–233 | Active statements from MON$STATEMENTS joined with MON$ATTACHMENTS; filter by state/user, limit ≤ 1000 |

## `src/introspection_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_table_stats` | 20–68 | Row counts: single table via `COUNT(*)`, all tables via `EXECUTE BLOCK` + dynamic `EXECUTE STATEMENT` |
| `handle_users` | 75–94 | DB owners + users with grants (RDB$DB_CREATORS FULL JOIN RDB$USER_PRIVILEGES) |
| `handle_grants` | 100–141 | GRANT privileges per table or all tables |
| `handle_blob_info` | 147–210 | BLOB columns: subtype (TEXT/BINARY) and length |
| `handle_index_usage` | 217–269 | Indexes with columns (LIST() window fn), uniqueness, type |

## `src/generator_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_generate_dev` | 14–189 | Generates dev code for up to 20 tables (java, go, rust, typescript, protobuf, openapi, graphql, python, lazarus, svelte). Special paths: Lazarus → `.lfm` forms; Svelte → Page + Form components; Python → `__init__.py` with cross-module imports. Rest goes through `generator::generate` + `save_or_inline`. Supports `compression` param (zstd/gzip/lz4/none) |
| `compression_suffix()` | 190–202 | Reads the `compression` arg and returns the file suffix (`.zst`, `.gz`, `.lz4`, or empty) |

## `src/view_dto_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_generate_view_dto` | 12–90 | Generates read-only DTOs for up to 20 views (java, go, rust, typescript, protobuf, openapi, graphql, python, json_schema); uses resolved view metadata. Supports `compression` param (zstd/gzip/lz4/none) | (java, go, rust, typescript, protobuf, openapi, graphql, python); uses resolved view metadata. Supports `compression` param (zstd/gzip/lz4/none) |

## `src/ddl_tools.rs`

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

## `src/migration_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_export` | 12–121 | Exports table/view/SQL to data file. Args: `source`, `output_path`, optional `format` (default "parquet": `parquet`/`ipc`/`csv`/`dbf`/`jsonl`, aliases `ndjson`/`ldjson` → `jsonl`), optional `compression` (validated per format: Parquet → snappy/zstd/lz4/none; IPC → zstd/lz4/none; CSV/DBF/JSONL → zstd/gzip/lz4/none, but extension takes precedence), optional `codepage` (DBF → cp437/cp850; CSV → cp437/cp850/iso-8859-1/windows-1252/utf-8; JSONL always UTF-8, codepage rejected), `database`, optional `on_success_event` / `on_fail_event` (POST_EVENT). Returns structured `{path, rows, columns, format, compression, download_url?}` — `download_url` present only in SSE mode when the file is inside the storage dir |
| `handle_import` | 122–205 | Imports data file into Firebird table. Args: `input_path`, `table`, `mode` (create/append), optional `format` (default "parquet"), `ignore_columns` (array), optional `codepage` (same validation as export), `database`, optional `on_success_event` / `on_fail_event` (POST_EVENT). Returns structured `{table, rows, columns, mode, format, ignored_columns, input_path, download_url?}` |
| `handle_describe_file` | 206–325 | Describes a file's schema without loading data. Args: `path`, optional `format` (default "parquet"), optional `codepage`. Returns Markdown (Parquet/IPC/DBF: types table; CSV/JSONL: columns + inferred types + 5 sample rows) + structured `{path, format, rows, columns, arrow_types, firebird_types, sample_rows, download_url?}`. **No DB connection needed** |
| `default_compression` | 326–332 | Returns default codec per format: Parquet → snappy, IPC → zstd, CSV/DBF/JSONL → none |
| `normalise_format` | 336–346 | Normalises a format string to its canonical name. Accepts aliases: `ndjson`/`ldjson` → `jsonl`, `dbase` → `dbf` |

## `src/quality_tools.rs`

| Function | Lines | Summary |
|---|---|---|
| `handle_data_quality` | 38–161 | `firebird_data_quality` handler. Args: `table` (required), `rules` (array, default all: `nulls`/`empty_strings`/`outliers`/`date_range`), `columns` (array, optional filter), `output_format` (`markdown`/`json`, default `markdown`), `sample_limit` (default 5, max 20), `database`. Returns `ToolResult` with structured `{table, rules, total_issues, findings[]}` |
| `run_nulls` | 168–209 | NULLs in NOT NULL columns (severity: error) |
| `run_empty_strings` | 210–249 | Empty strings in CHAR/VARCHAR/NVARCHAR columns (severity: warning) |
| `run_outliers` | 251–379 | IQR-based outliers in numeric columns (severity: warning). Q1/Q3 via `FIRST 1 SKIP`, bounds = [Q1−1.5·IQR, Q3+1.5·IQR]. Extra: `{q1, q3, iqr, lower_bound, upper_bound}` |
| `run_date_range` | 380–436 | Dates outside [1900-01-01, CURRENT_DATE+1] (severity: info). Skips TIME columns |
| `query_count` / `query_first_value` / `query_sample_values` | 437–471 | Thin wrappers over `query::query` for COUNT, first value, and sample values |
| `format_cell_value` | 472–475 | Renders a `Value` for report cells |
| `value_as_f64` | 477–485 | Coerces `Value` to `f64` (Float, Int, Bool, Text parsing) |
| `format_num` | 488–494 | Formats floats for SQL interpolation (avoids scientific notation) |
| `format_markdown` | 501–568 | Renders findings as a Markdown report (per-rule sections, tables) |

## `src/protocol.rs`

| Item | Lines | Summary |
|---|---|---|
| `ProtocolCore` | ~30–150 | Transport-agnostic protocol core. Holds `RwLock<Option<Config>>` (shared, reloadable) + `files_base_url: Option<String>` (SSE `/files` base URL, `None` in stdio). `new(cfg, files_base_url)`; `files_base_url()` getter. `handle_request(&Value) -> Option<Value>` routes JSON-RPC: `initialize` (returns protocol version + capabilities + serverInfo), `ping`, `tools/list` (via `tool_definitions()`), `tools/call` (via `dispatch()`, passing `files_base_url`). Returns `None` for notifications (no `id`). Thread-safe — designed to be shared across threads (one instance per SSE session, or a single instance behind an async runtime) |
| `PROTOCOL_VERSION` / `SERVER_NAME` / `SERVER_VERSION` | ~20–25 | Constants: `"2024-11-05"`, `"jed-personal-mcp"`, `"1.0.0"` |

## Conventions

- Handlers return `Result<String, String>` (Markdown text, error string) or
  `Result<ToolResult, String>` when they support structured output (generators,
  `create_database`, `metadata_extract` with `outputDirPath`).
- Table/column names are uppercased and validated with `is_valid_identifier`
  before being interpolated into SQL; user-supplied values are bound as
  parameters where possible (`query_with_param`).
- File output (generators) resolves dir via `storage_dir` and writes via
  `save_or_inline` (core `storage` module).
- **`download_url` (SSE only):** file-generating tools (`export`, `import`,
  `describe_file`, `generate_dev`, `generate_view_dto`, `metadata_extract` with
  `outputDirPath`, `list_files`) include a `download_url` in their structured
  response **only in SSE mode** (when `ProtocolCore.files_base_url` is set) and
  **only when the file is inside the storage dir**. The URL is
  `{base}/files?path=<url-encoded relative path>` (matching the `GET /files`
  endpoint in `mcp-stdio-bin`). In stdio mode the absolute `path` is the
  retrieval location and no `download_url` is emitted. Built by
  `common::file_download_url`. The URL is also appended to the human-readable
  `text` response (`Download:` for single-file tools, `Download URLs:` for
  multi-file tools) so agents that only read the text can retrieve the file.
- Logging goes through `mlog!` (core) to `{base}/logs/`.
- **Universal POST_EVENT:** every tool call accepts optional `on_success_event`
  / `on_fail_event` string params. After the handler completes, `dispatch()`
  calls `events::emit_tool_event()`, which: (1) appends a structured JSON line
  to `{base}/events.jsonl` (always), and (2) if the call targeted a valid
  database (resolved from the `database` arg or the configured default), opens
  a **fresh** connection and executes `POST_EVENT '<name>'` in its own
  transaction — notifying the Firebird event manager (clients registered via
  `SET EVENT` receive the notification on commit). Tools without a resolvable
  database (e.g. `firebird_get_databases`, `firebird_describe_file`) still get
  the JSON log line but no SQL event. Emission never fails the tool call — all
  errors are logged via `mlog!` and dropped. See `src/events.rs` and
  `mcp-core::query::post_event`.
- **No transport protocol code** — this crate is consumed by `mcp-stdio-bin`
  (both stdio and SSE transports).
