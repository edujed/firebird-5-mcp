# firebird-5-mcp — Project Summary (for AI agents)

> **Purpose:** token-efficient map of the project root. Read this first to
> locate a crate, script, or example before opening the full source.
>
> **What this project is:** a Firebird 5 MCP (Model Context Protocol) server
> written in Rust. It exposes Firebird database operations as MCP tools
> (query, schema introspection, DDL, codegen, statistics) over JSON-RPC 2.0.
> Supports two transports: **stdio** (default, line-delimited JSON-RPC over
> stdin/stdout) and **HTTP+SSE** (spec `2024-11-05`, for web UIs and
> non-stdio clients like llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode).
> Pure-Rust driver (`rsfbclient`) — no isql shell-out.

## Subdirectories

| Directory | AGENTS.md | Role |
|---|---|---|
| `crates/mcp-types/` | [`crates/mcp-types/AGENTS.md`](crates/mcp-types/AGENTS.md) | Shared value types (`Value`, `ColumnMeta` with `is_numeric` flag, `QueryResult`) + pure formatting helpers. No IO, no async, no driver code. |
| `crates/mcp-core/` | [`crates/mcp-core/AGENTS.md`](crates/mcp-core/AGENTS.md) | Core library — client, query, schema, DDL, metadata, stats, storage, config, codegen |
| `crates/mcp-tools/` | [`crates/mcp-tools/AGENTS.md`](crates/mcp-tools/AGENTS.md) | Shared MCP tool handlers (transport-agnostic) — handlers, `ToolResult`, `dispatch()` (with universal POST_EVENT), `tool_definitions()` |
| `crates/mcp-stdio-bin/` | [`crates/mcp-stdio-bin/AGENTS.md`](crates/mcp-stdio-bin/AGENTS.md) | MCP server binary (`firebird-5-mcp`) — stdio (default) + HTTP+SSE (`--sse`) transports, CLI flags, bearer-token auth |
| `crates/mcp-migration/` | [`crates/mcp-migration/AGENTS.md`](crates/mcp-migration/AGENTS.md) | Data file export/import — tables, views, SQL → Parquet/IPC/CSV/DBF/JSONL; Parquet/IPC/CSV/DBF/JSONL → tables (create/append). Compression: Snappy/ZSTD/LZ4/GZIP. Codepage: CP437/CP850 (DBF), UTF-8/ISO-8859-1/Win1252 (CSV). JSONL always UTF-8. Pure data-movement engines — POST_EVENT is universal in `mcp-tools` |
| `vendor/` | [`vendor/AGENTS.md`](vendor/AGENTS.md) | Vendored `rsfbclient-rust` 0.29.0 with NUMERIC/DECIMAL precision patch (see below) |
| `examples/` | [`examples/AGENTS.md`](examples/AGENTS.md) | Examples: complete test database (`test-mcp/`), Free Pascal client (`freePascal/`), CLI helpers (`mcp-call.*`) |
| `tests/` | [`tests/AGENTS.md`](tests/AGENTS.md) | Shell smoke tests — exercise the deployed binary over JSON-RPC (stdio) |

## `vendor/rsfbclient-rust/`

Vendored copy of `rsfbclient-rust` 0.29.0 with a **NUMERIC/DECIMAL precision
patch**. Upstream coerces scaled integers to `f64` (losing exactness —
`NUMERIC(18,4)` is the standard money type in Firebird schemas). The patch
keeps the scaled integer + scale in `xsqlda.rs` and renders the exact decimal
as text at fetch time in `wire.rs` (`scaled_to_decimal_string` /
`scaled_to_decimal_string_i128`). Applied via `[patch.crates-io]` in the
workspace `Cargo.toml`.

The vendor crate is a standalone workspace (its own `[workspace]` in
`Cargo.toml`) so its unit tests can run independently:
`cd vendor/rsfbclient-rust && cargo test --lib`.

## `scripts/`

| File | Lines | Role |
|---|---|---|
| `build.sh` | 167 | Local build — compiles Rust, installs to `~/.local/bin/firebird-5-mcp` (fixed name) |
| `publish_new_tag.sh` | 71 | Release helper — commit, tag, push to origin |

### `scripts/build.sh`

| Item | Lines | Summary |
|---|---|---|
| `PLATFORMS` array | 19–26 | 6 targets: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/{amd64,arm64} |
| `resolve_platforms()` | 29–59 | Maps short names (`linux`, `darwin`, `windows`, `all`, `current`) to platform lists; detects current OS/arch |
| `build_rust()` | 62–147 | Builds one platform: maps to Rust target triple, skips windows/arm64 (no stable GNU cross-target), skips darwin without osxcross, sets aarch64 linker, copies binary to `~/.local/bin/firebird-5-mcp` (fixed name, no platform suffix) |
| `main()` | 149–165 | Entry: resolves platforms, builds each, lists `~/.local/bin/` |

**Usage:**
```bash
./scripts/build.sh              # current platform → ~/.local/bin/firebird-5-mcp
./scripts/build.sh linux        # linux/amd64 + linux/arm64
./scripts/build.sh all          # all 6 platforms
```

> **Note:** the binary name is always `firebird-5-mcp` (no `-os-arch` suffix).
> Cross-compiled binaries overwrite each other in `~/.local/bin/` — this is
> intentional since local builds target the current platform only. Release
> artifacts with platform suffixes are built by GitHub Actions.

### `scripts/publish_new_tag.sh`

| Item | Lines | Summary |
|---|---|---|
| Usage check | 19–25 | Requires `<tag>` and `<commit-message>` args |
| Preflight | 34–54 | Checks: on a branch (not detached HEAD), tag doesn't exist, working tree has changes |
| Commit + tag + push | 59–67 | `git commit`, `git tag -a`, `git push origin` (branch + tag) |

**Usage:**
```bash
./scripts/publish_new_tag.sh v1.2.0 "feat: add new feature"
```

## Conventions

- **Five-crate workspace:** `mcp-stdio-bin` (thin transport layer — stdio + SSE) depends on `mcp-tools` (shared handlers + `ProtocolCore`) which depends on `mcp-core` (all Firebird logic) and `mcp-migration` (Parquet export/import). `mcp-types` holds shared value types. See each crate's `AGENTS.md` for function-level detail.
- **Two transports, one binary:** the `firebird-5-mcp` binary runs in **stdio** mode by default (JSON-RPC over stdin/stdout) or **SSE** mode with `--sse` (HTTP+SSE, spec `2024-11-05`). The protocol routing logic lives in `mcp-tools::ProtocolCore` (transport-agnostic); each transport is a thin layer in `mcp-stdio-bin` (`main.rs` for stdio, `sse.rs` for SSE). SSE sessions are UUID-keyed broadcast channels, reaped after 30 min of inactivity (configurable). Optional bearer-token auth (`--token` / `--token-file`), disabled by default.
- **`firebird_list_files`:** tool (in `mcp-tools::file_tools`) that lists generated files in the storage directory. Works in both transports — in stdio the client reads from the local filesystem; in SSE the client downloads via the `/files` HTTP endpoint.
- **File upload/download (SSE only):** `GET /files?path=<rel>` downloads a file from the storage directory (streaming, with path-traversal protection). `POST /files` accepts a multipart upload (`file` + `target` fields, 500 MB limit). Both use the same bearer-token auth as the SSE endpoints. The storage directory is resolved from the live config. See `crates/mcp-stdio-bin/src/files.rs`.
- **CORS:** all SSE endpoints support CORS for browser clients. Preflight `OPTIONS` requests return 204 with `Access-Control-Allow-Origin: *`. This allows web UIs hosted on different origins to communicate with the MCP server.
- **Build output:** binary lands in `~/.local/bin/firebird-5-mcp` (fixed name, no platform suffix). No `bin/` or `dist/` directories — GitHub Actions handles release artifacts.
- **Cross-compile caveats:** windows/arm64 is skipped (no stable Rust GNU target); darwin requires osxcross; aarch64-linux requires `aarch64-linux-gnu-gcc`.
- **Release flow:** `publish_new_tag.sh` commits + tags + pushes; GitHub Actions handles the rest.
- **Config:** the binary auto-loads `~/.firebird-mcp/config.json` (or `FIREBIRD_CONFIG` env, or `./config.json`). Tests rely on the default path — no env var needed.
- **Universal POST_EVENT:** every tool call accepts optional `on_success_event` / `on_fail_event` string params. After the handler completes, `dispatch()` (in `mcp-tools`) calls `events::emit_tool_event()`, which appends a JSON line to `~/.firebird-mcp/events.jsonl` and — when a valid database is resolved — opens a fresh connection and executes `POST_EVENT '<name>'` in its own transaction (notifying Firebird `SET EVENT` listeners on commit). Emission never fails the tool call. See `crates/mcp-tools/src/events.rs` and `crates/mcp-core/src/query/mod.rs::post_event`.
