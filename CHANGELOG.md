# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0]

### Added
- `arg_u32` helper in `mcp-bin` — numeric tool arguments now accept both JSON
  numbers and numeric strings, so LLMs that send `5` instead of `"5"` no longer
  fail. Applied to `max_rows`, `skip`, `limit`, `page_size`, `lock_wait`, and
  `state`.
- **Centralized path resolution** (`mcp-core/src/paths.rs`) — all file paths
  (config, output, logs) now derive from a single base directory:
  ```
  ~/.firebird-mcp/          (override: $FIREBIRD_MCP_HOME)
  ├── config.json           — server configuration
  ├── output/               — generated code files
  └── logs/                 — daily log files (yyyy-mm-dd.log)
  ```
- **File logging** — the MCP server now writes its logs to
  `~/.firebird-mcp/logs/yyyy-mm-dd.log` (one file per day).
- **`save_to` / output directory** — code generation tools
  (`firebird_generate_dev`, `firebird_generate_view_dto`) now write generated
  files to `~/.firebird-mcp/output/` by default (or to a caller-specified
  `save_to` path). The response contains a summary with file paths and
  previews instead of the full inline content.
- **`examples/test-mcp/`** — self-contained test database with 4 idempotent
  SQL scripts (`01_drop_schema.sql`, `02_create_schema.sql`,
  `03_create_triggers.sql`, `04_poulate.sql`) and a `create_test_db.sh`
  orchestrator. 12 tables, 2 views, 4 triggers, 2 procedures, 1 function,
  2 sequences, 5 domains, 122 rows of sample data.
- `firebird_table_stats` — per-table row counts. For a single table, uses
  `SELECT COUNT(*) FROM <table>`. For all tables, uses an `EXECUTE BLOCK` with `EXECUTE STATEMENT` to dynamically count each table.
- `firebird_users` — lists all users with access to the database, combining   `RDB$DB_CREATORS` (database owners) with `RDB$USER_PRIVILEGES` (users with explicit grants).
- `firebird_grants` — shows GRANTs for a specific table or for all tables, using `RDB$USER_PRIVILEGES` (available in all Firebird versions 2.5+).
- `firebird_blob_info` — lists BLOB columns with subtype (TEXT/BINARY), joining `RDB$RELATION_FIELDS` with `RDB$FIELDS` to get field type info.
- `firebird_index_usage` — shows all indexes with columns, uniqueness, and type. Uses `LIST()` window function to concatenate column names per index.
- **GraphQL code generation** — new `graphql` target for  `firebird_generate_dev` and `firebird_generate_view_dto`. Generates a GraphQL schema (types + Query) and a minimal Node.js/Express resolver (`graphql` + `express` + `cors`).
- **`GenOptions` struct** — code generation options unified into a single struct (`include_fks`, `database`, etc.) passed to all targets, replacing scattered individual parameters.
- **DTO unification** — `firebird_generate_view_dto` now shares the same `Target` trait and `GenOptions` as `firebird_generate_dev`, with `dto_type_for()` for read-only type resolution.
- **`examples/` folder** — ready-to-use examples for calling the MCP server outside an IDE:
  - `mcp-call.sh` / `mcp-call.bat` — shell scripts (Linux/macOS/Windows) that wrap the JSON-RPC call with a `python3` one-liner to extract `result.content[0].text`.
  - `freePascal/` — Free Pascal / Lazarus MCP client (`uMCP.pas` +   `test_mcp.pas`) using raw POSIX pipes (`fork`/`exec`/`read`/`write`) instead of `TProcess` (which has a limited API in FPC 3.2.2). 
    Tested on FPC 3.2.2 / Linux x86_64.
- **Editor registration docs** — README sections for VS CODE,  Cursor (`.cursor/mcp.json`) and JetBrains AI Assistant (`~/.jetbrains/mcp.json`) with `FIREBIRD_CONFIG` environment variable.

### Changed
- MCP stdio loop now handles `notifications/initialized` (acknowledged silently,
  no response) and other notifications, per the MCP spec. Malformed JSON lines
  are logged to stderr instead of being dropped silently.
- Tool schemas declare numeric arguments as `["number", "string"]` so clients
  and LLMs can send either form.
- **Config auto-resolution** — the binary now resolves `config.json`
  automatically from `~/.firebird-mcp/config.json` (or `$FIREBIRD_MCP_HOME`).
  The `FIREBIRD_CONFIG` environment variable is no longer needed. Example
  scripts (`mcp-call.sh`, `mcp-call.bat`, `freePascal/uMCP.pas`) updated to
  remove the obsolete `MCP_CONFIG` reference.
- **`firebird_get_databases`** now reloads `config.json` from disk on every
  call, so newly registered databases (via `firebird_create_database` with
  `register=true`) appear immediately without restarting the server.
- Workspace moved to repo root (removed `firebird-5-mcp/` subfolder).
- Go build support removed from `scripts/build.sh` (Go version discontinued).

### Fixed
- `firebird_run_script` with a `path` argument now enforces the configured
  `allowed_paths` (same containment rule as `firebird_create_database`),
  preventing the server from reading arbitrary files off the host.
- **Crash on empty databases list** — `Config::default_database()` panicked
  when the `databases` array was empty. Now returns an empty string safely.

## [1.1.0] — Pure Rust (firebird-5-mcp)

### Added
- `firebird_crypt_state` — at-rest encryption state of the database
  (`MON$DATABASE.MON$CRYPT_STATE`, Firebird 3.0+).
- `firebird_database_stats` — full `MON$DATABASE` snapshot with live
  attachments (`MON$ATTACHMENTS`) and active statements
  (`MON$STATEMENTS`).
- `firebird_query_history` — active statements from `MON$STATEMENTS`
  joined with `MON$ATTACHMENTS` (user, remote address, state filter).
- `page_size` parameter on `firebird_create_database` (default 8192;
  valid: 1024, 2048, 4096, 8192, 16384, 32768).
- `isolation` and `lock_wait` parameters on `firebird_run_script`
  (4 presets: read-committed, snapshot, consistency, readonly).
- `tests/` — shell scripts (`.sh` + `.bat`) for smoke-testing the
  deployed binary.
- `.github/workflows/release.yml` — CI/CD release workflow: builds Rust
  for 5 platforms (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64,
  windows/amd64) and uploads archives to GitHub Releases on tag push.

### Changed
- **Codegen refactor** — the code generation module was restructured for
  DRY / Clean Code / SOLID:
  - New `FbType` enum replaces the old `ResolvedType` struct and 11
    free-standing `is_*` predicate functions. Each target does an
    exhaustive `match` over it (compiler guarantees no type is forgotten).
  - Per-language target files (`targets/java.rs`, `targets/go.rs`, etc.)
    behind a `trait Target` with `type_for()`, `generate()`, and
    `dto_type_for()` methods.
  - UI targets (Lazarus `.lfm` forms, Svelte 5 pages) moved to
    `codegen/ui/` subfolder.
  - `view_dto.rs` rewritten to use `FbType` + `dto_type_for()` — removed
    6 duplicated `fb_type_to_*` string-matching functions (~200 lines).
  - Shared naming helpers (`to_pascal_case`, `to_snake_case`, `to_camel_case`)
    extracted to `codegen/naming.rs` with unit tests.
- **Module split** — large files split into sub-modules (public API
  unchanged via re-exports):
  - `query.rs` (850 lines) → `query/mod.rs` + `query/split.rs` +
    `query/row_limit.rs` + `query/batch.rs`
  - `schema.rs` (450 lines) → `schema/mod.rs` + `schema/markdown.rs`
- **BLOB sub_type fix** — `types.rs` had the BLOB sub_type mapping
  inverted. Correct mapping (per Firebird docs): `sub_type 1 = TEXT`,
  `sub_type 2 = BINARY`. Fixed in `format_column_type`.
- `scripts/build.sh` — fixed 4 bugs:
  - `DIST_DIR` undefined variable (used `${ROOT}/dist` instead).
  - Duplicate `.exe` suffix in Windows binary name.
  - Linux/arm64 cross-compile now works (sets
    `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` env var).
  - Windows binary copy now finds `firebird-5-mcp.exe` in target dir.

### Fixed
- `explain_plan` SQL injection: the user's SQL is now passed as a bind
  parameter instead of being interpolated into the `MON$STATEMENTS`
  query.
- FK resolution in `describe_table`: now joins `RDB$INDEX_SEGMENTS` to
  correctly resolve foreign key columns.
- All write paths (DML, DDL, scripts, batch inserts) now use explicit
  read-write transactions (`begin_rw_transaction()`).

## [1.0.1] — Rust port (firebird-5-mcp)

### Added
- **Rust port of the MCP server** (`firebird-5-mcp/`) using the pure-Rust
  [rsfbclient](https://crates.io/crates/rsfbclient) driver — no `isql`, no
  ODBC, no native libraries required.
- Workspace split into a reusable library (`firebird-5-mcp-core`) and the
  MCP server binary (`firebird-5-mcp`), so the library can be reused in
  other Rust projects (e.g. desktop apps).
- `firebird_explain_plan` — get the execution plan of a statement. The
  statement is executed inside a transaction that is always rolled back,
  so it is safe to preview expensive queries (e.g. nested DELETEs).
- `firebird_create_procedure`, `firebird_create_function`,
  `firebird_create_sequence` — DDL without `isql` (no `SET TERM` needed).
- `firebird_describe` — multi-object describe (tables, views, procedures,
  functions, triggers, sequences, domains) in a single call.
- `firebird_find_column` — find columns by name pattern (LIKE) across all
  tables and views.
- `firebird_check_duplicates` — check for duplicate values in one or more
  columns (composite keys supported) before creating a unique index or
  constraint.
- `firebird_describe_database` — **Health** section (gstat-like) from
  `MON$DATABASE` / `MON$ATTACHMENTS`: ODS version, page size, file size,
  dialect, shutdown mode, read-only, forced writes, buffer pool, oldest
  transactions and active user connections.
- `firebird_generate_dev` — code generation from table schemas (up to 20
  tables per call):
  - Java (JPA + Lombok), Go (sqlx), Rust (rsfbclient `FromRow`),
    TypeScript (interfaces), Protobuf (Connect-RPC/gRPC),
    OpenAPI 3.2.0 (YAML), Lazarus/Delphi (`.lfm` forms with TFBDataSet),
    Svelte 5 (Page + Form using `@edujed/jedsvelted-ui`).
  - Optional `include_fks` parameter generates named FK relations
    (Java `@ManyToOne`) or FK comments (Go/Rust/TS).
- `firebird_generate_view_dto` — DTO/record generation from views using
  resolved metadata (`SELECT FIRST 1 * FROM view`), read-only output,
  same 8 formats as `firebird_generate_dev` (Svelte generates a
  read-only Page).
- `scripts/build.sh` — builds both the Go and Rust versions for all
  supported platforms and packages the binaries into `dist/`
  (Windows as `.zip`, Linux as `.tar.gz`).
- Release profile optimized for size: `opt-level = 3`, `codegen-units = 1`,
  `lto = "fat"`, `panic = "abort"`, `strip = true` — ~1.2MB binary
  (vs ~8MB for the Go version).

### Changed
- Project renamed: `jed-personal-mcp/` → `firebird-go-mcp/`,
- included folder `firebird-5-mcp/`.
- `config.json` format kept fully compatible with the Go version.
- `firebird_metadata_extract` now uses the system tables directly
  (replaces `isql -x`), so the Go version's `isql_path` setting is not
  required by the Rust version.

### Removed
- Firebird events support (not ported — events must be registered in
  advance and are not a good fit for the MCP use case).

## [1.0.0] — Go version (jed-personal-mcp)

### Added
- Initial release of `jed-personal-mcp`: MCP server for Firebird written
  in Go, with 15 tools (query, count, sample, insert batch, run script,
  describe table/database, list, get/create database, create trigger,
  execute immediate, alter column type, drop, metadata extract via
  `isql -x`).
