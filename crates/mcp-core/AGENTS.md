# mcp-core — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-core`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this crate is:** the `firebird-5-mcp-core` library. All Firebird logic
> lives here: connection handling, query execution, schema introspection, DDL,
> metadata extraction, MON$ statistics, file storage, config, logging, and code
> generation. The sibling crates `firebird-5-mcp-tools` (`crates/mcp-tools`) and
> `firebird-5-mcp` (`crates/mcp-stdio-bin`) are thin layers on top of this one.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | — | Package `firebird-5-mcp-core`; deps: `rsfbclient`, `rsfbclient-core`, `serde`, `serde_json`, `chrono`, `dirs` |
| `src/lib.rs` | 35 | Module declarations + re-exports (`FirebirdClient`, `IsolationPreset`, `Config`, `DatabaseConfig`, `ServerConfig`) |
| `src/client.rs` | 236 | `FirebirdClient` connection wrapper (pure-Rust `rsfbclient` driver) |
| `src/config.rs` | 302 | `config.json` loading/validation/registration |
| `src/query/mod.rs` | 183 | Query execution: `query`, `query_with_limit`, `query_with_param`, `exec`, `execute` (types re-exported from `mcp-types`) |
| `src/query/batch.rs` | 164 | `run_script`, `insert_batch`, `count`, `sample` |
| `src/query/row_limit.rs` | 193 | `FIRST`/`SKIP` clause injection |
| `src/query/split.rs` | 562 | SQL statement splitting (`SET TERM`, `BEGIN`/`END`) |
| `src/schema/mod.rs` | 363 | Schema introspection (RDB$ tables) |
| `src/schema/markdown.rs` | 92 | Markdown rendering of `TableSchema` |
| `src/ddl.rs` | 144 | DDL: triggers, procedures, functions, sequences, drops |
| `src/explain.rs` | 74 | Execution plan retrieval (execute + rollback) |
| `src/metadata.rs` | 891 | Full DDL extraction (replaces `isql -x`) |
| `src/stats.rs` | 354 | MON$ statistics + encryption state |
| `src/storage.rs` | 396 | File storage for generated artifacts |
| `src/types.rs` | 10 | Re-exports from `firebird-5-mcp-types` (backward compat) |
| `src/paths.rs` | 104 | Centralized path resolution |
| `src/log.rs` | 87 | File-based daily logging + `mlog!` macro |
| `src/codegen/mod.rs` | 43 | Codegen re-exports |
| `src/codegen/fbtype.rs` | 494 | `FbType` enum (typed Firebird column types) |
| `src/codegen/generator.rs` | 129 | `Language`, `GenOptions`, `generate` dispatcher |
| `src/codegen/naming.rs` | 78 | `to_pascal_case`, `to_snake_case`, `to_camel_case` |
| `src/codegen/view_dto.rs` | 193 | View DTO generation |
| `src/codegen/targets/mod.rs` | 88 | `Target` trait + `GeneratedFile` |
| `src/codegen/targets/java.rs` | 204 | Java (JPA + Lombok) target |
| `src/codegen/targets/go.rs` | 158 | Go (sqlx) target |
| `src/codegen/targets/rust.rs` | 152 | Rust (sqlx) target |
| `src/codegen/targets/typescript.rs` | 104 | TypeScript target |
| `src/codegen/targets/protobuf.rs` | 106 | Protobuf target |
| `src/codegen/targets/openapi.rs` | 130 | OpenAPI 3.2.0 target |
| `src/codegen/targets/graphql.rs` | 680 | GraphQL (schema + resolvers) target |
| `src/codegen/ui/mod.rs` | 9 | UI targets sub-module |
| `src/codegen/ui/lazarus.rs` | 218 | Lazarus/Delphi `.lfm` form generation |
| `src/codegen/ui/svelte.rs` | 394 | Svelte 5 page/form generation |

## `src/client.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct FirebirdClient` | 15–20 | Connection wrapper: `path`, `user`, `password`, `Connection` |
| `metadata_transaction_conf()` | 28–34 | Read-only SNAPSHOT config for metadata reads |
| `rw_transaction_conf()` | 37–43 | Read-write transaction config |
| `enum IsolationPreset` | 57–74 | `ReadCommited`, `Snapshot`, `Consistency`, `ReadOnly` |
| `IsolationPreset::parse` | 79–87 | String → preset (case-insensitive) |
| `IsolationPreset::label` | 90–97 | Preset → human label |
| `IsolationPreset::to_conf` | 101–128 | Preset → `TransactionConf` |
| `FirebirdClient::new` | 133–148 | Connects with read-only SNAPSHOT default txn |
| `FirebirdClient::create_database` | 156–175 | Creates a new `.fdb` file |
| `FirebirdClient::db_path` | 178–180 | Returns the database path |
| `FirebirdClient::second_connection` | 186–195 | Opens a second connection (for explain_plan MON$ reads) |
| `FirebirdClient::execute_immediate` | 202–206 | Executes SQL containing `SET TERM` blocks |
| `FirebirdClient::begin_rw_transaction` | 214–217 | Starts an explicit RW transaction |
| `FirebirdClient::begin_transaction` | 227–235 | Starts a transaction with a given `IsolationPreset` |

## `src/config.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct DatabaseConfig` | 31–36 | `{ name, path, user, password }` |
| `struct ServerConfig` | 40–55 | Server-level config: databases, allowed_paths, storage |
| `struct Config` | 59–65 | Top-level config wrapper |
| `Config::load` | 71–80 | Loads from disk; `None` if missing |
| `Config::load_or_create` | 88–120 | Loads or creates default; never overwrites existing file |
| `Config::default_empty` | 123–137 | Empty config with defaults |
| `Config::save` | 140–143 | Saves to the configured path |
| `Config::save_to` | 146–155 | Saves to an explicit path |
| `Config::register_database` | 162–194 | Adds a database entry; rejects duplicates |
| `Config::validate` | 196–219 | Validates paths, names, allowed_paths |
| `Config::default_database` | 223–229 | Returns the default (first) database |
| `Config::reload` | 236–243 | Re-reads from disk |
| `ServerConfig::find_database` | 248–258 | Looks up a database by name |
| `ServerConfig::database_names` | 261–267 | Lists all database names |
| `ServerConfig::is_path_allowed` | 270–279 | Checks if a path is within `allowed_paths` |
| `config_path()` | 288–297 | `FIREBIRD_CONFIG` env → `./config.json` → `{base}/config.json` |
| `default_config_path()` | 300–302 | Returns `{base}/config.json` |

## `src/query/mod.rs`

Re-exports `Value`, `ColumnMeta`, `QueryResult` from `firebird-5-mcp-types`.

| Item | Lines | Summary |
|---|---|---|
| `query()` | 28–30 | Executes a SELECT, returns `QueryResult` |
| `query_with_limit()` | 33–49 | SELECT with `FIRST`/`SKIP` injection |
| `query_with_param()` | 52–62 | SELECT with a bound parameter |
| `query_unbounded()` | 65–78 | SELECT with **no** row-limit injection — for bulk data movement (file export); never for LLM-facing queries |
| `rows_to_result()` | 81–101 | Converts raw rows to `QueryResult` |
| `sql_type_name()` | 111–135 | Raw ISC_* code → type name string |
| `row_value()` | 138–150 | Raw cell → `Value` enum |
| `exec()` | 160–179 | DML/DDL in RW transaction, commit/rollback |
| `execute()` | 183–199 | SELECT-first, DML/DDL fallback |

## `src/query/batch.rs`

| Function | Lines | Summary |
|---|---|---|
| `run_script()` | 25–54 | Splits multi-statement script, executes in single transaction |
| `insert_batch()` | 58–109 | Bulk INSERT via prepared statement, single transaction |
| `json_value_to_sql()` | 112–130 | JSON value → SQL literal |
| `count()` | 133–148 | `COUNT(*)` on a table, optional WHERE |
| `sample()` | 151–164 | First N rows of a table |

## `src/query/row_limit.rs`

| Item | Lines | Summary |
|---|---|---|
| `accepts_row_limit()` | 7–11 | Returns true for SELECT/WITH only |
| `DEFAULT_ROWS` const | 16 | = 100 |
| `MAX_ROWS` const | 20 | = 1000 |
| `inject_row_limit()` | 31–77 | Inserts `FIRST N SKIP M` into a SELECT |
| `find_select_insertion_point()` | 83–110 | Finds the right position for `FIRST`/`SKIP` |

## `src/query/split.rs`

| Function | Lines | Summary |
|---|---|---|
| `split_statements()` | 13–19 | Public entry: splits SQL into statements |
| `contains_set_term()` | 22–61 | Detects `SET TERM` directives |
| `skip_string()` | 65–79 | Skips over a quoted string |
| `skip_line_comment()` | 83–93 | Skips `--` line comments |
| `skip_block_comment()` | 97–106 | Skips `/* */` block comments |
| `extract_word()` | 110–121 | Extracts the next identifier/keyword |
| `split_without_set_term()` | 125–189 | Splits on `;` respecting strings/comments |
| `strip_comments()` | 192–242 | Removes all comments from SQL |
| `split_with_set_term()` | 248–388 | Splits using `SET TERM` delimiters |
| `parse_set_term_directive()` | 392–442 | Parses `SET TERM` to get the delimiter |
| tests | 445–562 | Unit tests for splitting |

## `src/schema/mod.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct TableInfo` | 15–19 | `{ name, is_view }` |
| `struct ColumnInfo` | 23–34 | Column metadata (name, type, size, nullability, default, comment) |
| `struct IndexInfo` | 38–43 | `{ name, columns, unique }` |
| `struct ConstraintInfo` | 47–50 | `{ name, type, columns }` |
| `struct TriggerInfo` | 54–58 | `{ name, type, timing }` |
| `struct TableSchema` | 62–69 | Full schema: table, columns, indexes, constraints, triggers, comments |
| `struct ForeignKeyInfo` | 73–79 | FK metadata |
| `list_tables()` | 82–104 | Lists all tables/views from RDB$ |
| `describe_table()` | 107–247 | Full schema of one table/view |
| `get_foreign_keys()` | 250–280 | FKs for a table |
| `get_comments()` | 283–323 | Column/table comments |
| `text_at()` | 327–338 | Safe text cell accessor |
| `int_at()` | 340–348 | Safe int cell accessor |
| `opt_int_at()` | 350–356 | Optional int cell accessor |
| `opt_text_at()` | 358–363 | Optional text cell accessor |

## `src/schema/markdown.rs`

| Function | Lines | Summary |
|---|---|---|
| `format_schema_markdown()` | 6–94 | Renders `TableSchema` as Markdown |

## `src/ddl.rs`

| Function | Lines | Summary |
|---|---|---|
| `create_trigger()` | 13–40 | Creates a trigger |
| `create_procedure()` | 44–61 | Creates a stored procedure |
| `create_function()` | 65–83 | Creates a UDF |
| `create_sequence()` | 86–90 | Creates a sequence/generator |
| `drop()` | 93–111 | Drops an object by type and name |
| `alter_column_type()` | 114–122 | Changes a column's type to a domain |
| `exec_block()` | 130–144 | RW transaction wrapper for DDL |

## `src/explain.rs`

| Function | Lines | Summary |
|---|---|---|
| `explain_plan()` | 25–74 | Executes statement in rolled-back txn, reads MON$STATEMENTS for plan |

## `src/metadata.rs`

| Function | Lines | Summary |
|---|---|---|
| `extract_ddl()` | 14–16 | Extracts all DDL |
| `extract_ddl_filtered()` | 20–40 | Extracts DDL for specific objects |
| `should_extract()` | 42–47 | Checks if an object should be extracted |
| `extract_domains()` | 49–105 | Domain DDL |
| `extract_tables()` | 107–282 | Table DDL (columns, constraints, indexes) |
| `extract_views()` | 284–311 | View DDL |
| `extract_sequences()` | 313–341 | Sequence DDL |
| `extract_triggers()` | 343–387 | Trigger DDL |
| `extract_procedures()` | 389–456 | Procedure DDL |
| `resolve_domain_type()` | 459–485 | Resolves domain type to base type |
| `decode_blr_default()` | 490–528 | Decodes BLR-encoded default values |
| `extract_functions()` | 530–640 | Function DDL |
| `describe_objects()` | 646–850 | Describes multiple objects by name |
| `text_at()` | 854–864 | Safe text cell accessor |
| `opt_text_at()` | 867–872 | Optional text cell accessor |
| `opt_int_at()` | 875–881 | Optional int cell accessor |
| `int_at()` | 883–891 | Safe int cell accessor |

## `src/stats.rs`

| Item | Lines | Summary |
|---|---|---|
| `enum CryptState` | 24–33 | Encryption states |
| `CryptState::from_raw` | 37–44 | Raw int → `CryptState` |
| `CryptState::label` | 47–54 | `CryptState` → human label |
| `CryptState::is_encrypted` | 57–59 | Whether the DB is encrypted |
| `crypt_state()` | 66–70 | Reads MON$DATABASE encryption state |
| `engine_version()` | 73–82 | Reads engine version |
| `engine_major()` | 85–92 | Major version number |
| `struct MonDatabase` | 95–118 | MON$DATABASE snapshot |
| `struct AttachmentInfo` | 121–131 | MON$ATTACHMENTS row |
| `struct StatementInfo` | 134–140 | MON$STATEMENTS row |
| `struct DatabaseStats` | 143–147 | Aggregated stats |
| `read_mon_database()` | 150–196 | Reads MON$DATABASE |
| `read_mon_attachments()` | 199–230 | Reads MON$ATTACHMENTS |
| `read_mon_statements()` | 233–253 | Reads MON$STATEMENTS |
| `database_stats()` | 256–265 | Aggregates all MON$ data |
| `struct QueryHistoryEntry` | 269–277 | Query history row |
| `query_history()` | 286–339 | Active statements with user/remote info |
| `cell_text()` | 337–345 | Safe text cell accessor |
| `cell_int()` | 347–354 | Safe int cell accessor |

## `src/storage.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct StorageConfig` | 13–16 | `{ dir: String }` (empty = use default) |
| `StorageConfig::effective_dir` | 23–29 | Resolves the effective storage directory |
| `default_storage_dir()` | 35–37 | Returns `~/.firebird-mcp/output/` |
| `struct StorageResult` | 41–46 | `{ output_dir: String, files: Vec<WrittenFile> }` |
| `struct WrittenFile` | 50–61 | `{ name, path, size, lines, format }` |
| `resolve_output_dir()` | 68–81 | Resolves output dir, rejects path traversal |
| `write_files()` | 87–124 | Writes generated files to disk |
| `preview()` | 131–148 | Builds a preview (first 10 + last 5 lines) |
| `summary()` | 163–196 | Builds a summary of written files |
| `format_size()` | 199–209 | Bytes → human-readable size |
| `truncate_lines()` | 211–213 | Truncates a list of lines |
| `truncate_line()` | 215–226 | Truncates a single line |
| `to_snake_case()` | 232–269 | Converts a string to snake_case |
| tests | 271–396 | Unit tests for storage |

## `src/types.rs`

Re-exports from `firebird-5-mcp-types` for backward compatibility:
`Value`, `ColumnMeta`, `QueryResult`, `format_cell`, `format_column_type`,
`is_valid_identifier`. New code should depend on `mcp-types` directly.

## `src/paths.rs`

| Item | Lines | Summary |
|---|---|---|
| `ENV_HOME` const | 19 | `FIREBIRD_MCP_HOME` |
| `BASE_DIR_NAME` const | 22 | `.firebird-mcp` |
| `base_dir()` | 30–37 | `FIREBIRD_MCP_HOME` → `~/.firebird-mcp` → `/tmp/firebird-mcp` |
| `config_file()` | 40–42 | Returns `{base}/config.json` |
| `output_dir()` | 45–47 | Returns `{base}/output/` |
| `logs_dir()` | 50–52 | Returns `{base}/logs/` |
| `ensure_dirs()` | 62–68 | Creates base/output/logs dirs if missing |
| tests | 70–104 | Unit tests for paths |

## `src/log.rs`

| Item | Lines | Summary |
|---|---|---|
| `LOG_DIR` static | 21 | OnceLock for the log directory |
| `init()` | 27–29 | Initializes the log directory |
| `log_dir()` | 32–34 | Returns the log directory |
| `write_line()` | 41–59 | Writes a line to today's log file |
| `mlog!` macro | 68–72 | Logging macro: `mlog!("message {}", arg)` |
| tests | 74–87 | Unit tests for logging |

## `src/codegen/mod.rs`

Re-exports (L30–43): `FbType`, `GenOptions`, `Language`, `generate`, `to_camel_case`, `to_pascal_case`, `to_snake_case`, `Target`, plus UI re-exports (`LazarusField`, `lazarus_from_column_info`, `lazarus_from_view_field`, `generate_lfm`, `SvelteField`, `svelte_from_column_info`, `svelte_from_view_field`, `generate_form`, `generate_page`, `generate_page_for_view`) and view_dto re-exports (`ViewField`, `generate_view_dto`, `get_view_metadata`). `GeneratedFile` is NOT re-exported (it lives in `codegen::targets`).

## `src/codegen/fbtype.rs`

| Item | Lines | Summary |
|---|---|---|
| `enum FbType` | 18–59 | Typed Firebird column types (compiler-enforced exhaustiveness) |
| `FbType::from_column` | 66–111 | `ColumnInfo` → `FbType` |
| `FbType::raw` | 116–141 | `FbType` → raw type code |
| `FbType::type_str` | 146–149 | `FbType` → type string |
| Category predicates | 153–226 | `is_blob`, `is_numeric`, `is_string`, `is_temporal`, etc. |
| `FbType::from_type_str` | 234–300 | Type string → `FbType` |
| `parse_size()` | 304–306 | Parses size from type string |
| `parse_precision_scale()` | 309–320 | Parses precision/scale from type string |
| tests | 322–494 | Unit tests for `FbType` |

## `src/codegen/generator.rs`

| Item | Lines | Summary |
|---|---|---|
| `enum Language` | 17–27 | `Java`, `Go`, `Rust`, `TypeScript`, `Protobuf`, `OpenApi`, `Graphql`, `Lazarus`, `Svelte` |
| `Language::parse` | 30–43 | String → `Language` |
| `struct GenOptions` | 48–63 | `{ language, include_fks, include_comments, include_examples, package, is_view }` |
| `GenOptions::new` | 67–76 | Constructor |
| `generate()` | 84–129 | Dispatcher: language → target, calls `Target::generate`; `Lazarus`/`Svelte` handled inline (placeholder files) |

## `src/codegen/naming.rs`

| Function | Lines | Summary |
|---|---|---|
| `to_pascal_case()` | 7–15 | Converts to PascalCase |
| `to_snake_case()` | 18–24 | Converts to snake_case |
| `to_camel_case()` | 27–50 | Converts to camelCase |
| tests | 52–78 | Unit tests for naming |

## `src/codegen/view_dto.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct ViewField` | 28–32 | `{ name, type_str, nullable }` |
| `get_view_metadata()` | 41–65 | `SELECT FIRST 1 * FROM view` to resolve columns |
| `infer_type_from_value()` | 69–81 | Infers type string (`String`) from a sample value |
| `generate_view_dto()` | 84–101 | Generates DTOs for multiple views |
| `generate_single_view_dto()` | 105–159 | Generates a single view DTO |
| `view_to_schema()` | 164–193 | Converts view fields to a synthetic `TableSchema` |

## `src/codegen/targets/mod.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct GeneratedFile` | 37–41 | `{ name, content, format }` |
| `GeneratedFile::new` | 45–55 | Constructor |
| `trait Target` | 62–88 | `type_for`, `generate` (returns `Vec<GeneratedFile>`), `dto_type_for` — implemented per language |

## `src/codegen/targets/java.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct JavaTarget` | 9 | JPA + Lombok target |
| `type_for` | 12–32 | `FbType` → Java type |
| `generate` | 34+ | Generates Java entity/DTO |

## `src/codegen/targets/go.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct GoTarget` | 9 | sqlx target |
| `type_for` | 16–35 | `FbType` → Go type |
| `dto_type_for` | 37–39 | DTO type mapping |
| `generate` | 41+ | Generates Go struct |

## `src/codegen/targets/rust.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct RustTarget` | 9 | sqlx target |
| `type_for` | 16–37 | `FbType` → Rust type |
| `dto_type_for` | 39–41 | DTO type mapping |
| `generate` | 43+ | Generates Rust struct |

## `src/codegen/targets/typescript.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct TypeScriptTarget` | 9 | TypeScript target |
| `type_for` | 16–33 | `FbType` → TS type |
| `generate` | 35+ | Generates TS interface |

## `src/codegen/targets/protobuf.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct ProtobufTarget` | 9 | Protobuf target |
| `type_for` | 16–34 | `FbType` → protobuf type |
| `generate` | 36+ | Generates `.proto` file |

## `src/codegen/targets/openapi.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct OpenApiTarget` | 9 | OpenAPI 3.2.0 target |
| `type_for` | 17–22 | `FbType` → OpenAPI type |
| `generate` | 24+ | Generates OpenAPI schema |

## `src/codegen/targets/graphql.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct GraphqlTarget` | 13 | GraphQL (schema + resolvers) target |
| `type_for` | 21–41 | `FbType` → GraphQL type |
| `generate` | 43+ | Produces 2 files: schema + resolvers |

## `src/codegen/ui/lazarus.rs`

| Item | Lines | Summary |
|---|---|---|
| Layout consts | 13–19 | `MARGEM_ESQUERDA`, `TOP_INICIAL_CAMPOS`, `ALTURA_LABEL`, `ALTURA_EDIT`, `ESPACO_ENTRE_ELEMENTOS`, `ESPACO_ENTRE_CAMPOS`, `LARGURA_PADRAO_EDIT` |
| `struct LazarusField` | 23–28 | `{ name, type_str, size, nullable }` |
| `generate_lfm()` | 31–126 | `generate_lfm(object_name: &str, fields: &[LazarusField], is_view: bool) -> String` |
| `determine_component()` | 129–176 | `determine_component(type_str: &str, size: i64) -> (&'static str, i32)` — component type + width |
| `from_column_info()` | 179–193 | `from_column_info(col: &ColumnInfo) -> LazarusField` |
| `from_view_field()` | 196–205 | `from_view_field(view_field: &ViewField) -> LazarusField` |
| `parse_size_from_type()` | 208–218 | `parse_size_from_type(type_str: &str) -> i64` |

## `src/codegen/ui/svelte.rs`

| Item | Lines | Summary |
|---|---|---|
| `struct SvelteField` | 14–21 | `{ name, type_str, size, nullable, is_pk, is_fk }` |
| `generate_page()` | 24–205 | `generate_page(table_name: &str, fields: &[SvelteField], pk_field: &str, fk_fields: &[String]) -> String` |
| `generate_form()` | 208–303 | `generate_form(_table_name: &str, fields: &[SvelteField]) -> String` |
| `capitalize()` | 306–312 | `capitalize(s: &str) -> String` |
| `format_title()` | 315–323 | `format_title(name: &str) -> String` |
| `default_value_for_type()` | 326–339 | `default_value_for_type(type_str: &str) -> &'static str` |
| `rust_type_to_ts()` | 342–356 | `rust_type_to_ts(type_str: &str) -> &'static str` — Firebird type → TS type |
| `from_column_info()` | 359–375 | `from_column_info(col: &ColumnInfo, is_pk: bool, is_fk: bool) -> SvelteField` |
| `from_view_field()` | 378–387 | `from_view_field(vf: &ViewField) -> SvelteField` |
| `generate_page_for_view()` | 390–394 | `generate_page_for_view(view_name: &str, fields: &[SvelteField]) -> String` |

## Conventions

- **Pure-Rust driver** (`rsfbclient`), no isql shell-out.
- **Connection** created with read-only SNAPSHOT default txn; explicit RW txn needed for DML/DDL.
- **`FbType` enum** guarantees exhaustive type mapping per target (compiler-enforced).
- **View DTOs** reuse `Target::generate` via synthetic `TableSchema`.
- **Path traversal** rejected in storage; `allowed_paths` gates file access.
- **Logging** via `mlog!` to `{base}/logs/yyyy-mm-dd.log`.
- **Config lookup:** `FIREBIRD_CONFIG` env → `./config.json` → `~/.firebird-mcp/config.json`.
