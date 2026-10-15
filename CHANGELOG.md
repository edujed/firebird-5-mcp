# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Rust port (firebird-5-mcp)

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
- Project renamed: `golang/` → `firebird-go-mcp/`,
  `firebird-mcp-workspace/` → `firebird-5-mcp/`.
- `config.json` format kept fully compatible with the Go version.
- `firebird_metadata_extract` now uses the system tables directly
  (replaces `isql -x`), so the Go version's `isql_path` setting is not
  required by the Rust version.

### Removed
- Firebird events support (not ported — events must be registered in
  advance and are not a good fit for the MCP use case).

## [1.0.0] — Go version (firebird-go-mcp)

### Added
- Initial release of `jed-personal-mcp`: MCP server for Firebird written
  in Go, with 15 tools (query, count, sample, insert batch, run script,
  describe table/database, list, get/create database, create trigger,
  execute immediate, alter column type, drop, metadata extract via
  `isql -x`).
