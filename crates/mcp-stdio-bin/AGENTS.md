# mcp-stdio-bin — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-stdio-bin`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this crate is:** the MCP stdio server binary (`firebird-5-mcp`). It speaks
> JSON-RPC 2.0 (line-delimited) over stdin/stdout and exposes Firebird tools. All
> tool handlers live in the sibling crate `firebird-5-mcp-tools` (`crates/mcp-tools`) —
> this crate is a thin layer of protocol loop + config management.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 13 | Package `firebird-5-mcp`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-tools`, `serde_json` |
| `src/main.rs` | 151 | Entry point, MCP protocol loop |

## `src/main.rs`

| Function | Lines | Summary |
|---|---|---|
| `main()` | 25–151 | Loads `Config`, inits file logging, then loops over stdin lines: parses JSON-RPC, handles `initialize` / `ping` / `tools/list` / `tools/call` (via `dispatch` from `mcp-tools`), writes line-delimited responses. Notifications (no `id`) are acked silently. |

Key statics: `CONFIG: RwLock<Option<Config>>` (L19), `SERVER_NAME`/`SERVER_VERSION`/`PROTOCOL_VERSION` (L21–23).

`build_time()` is imported from `mcp-tools` (single source of truth).

## What lives in `mcp-tools` (sibling crate)

All tool handlers, `ToolResult`, `dispatch()`, and `tool_definitions()` live in
`crates/mcp-tools/`. See that crate's `AGENTS.md` for the full handler map.

## Conventions

- This crate contains **no tool handlers** — they live in `mcp-tools`.
- The protocol loop is the only logic here: parse JSON-RPC, route to
  `mcp-tools::dispatch`, format the response.
- Config is loaded once at startup and stored in a `RwLock` so that
  `firebird_get_databases` can reload it and `firebird_create_database`
  can update it in-memory.
- Logging goes through `mlog!` (core) to `{base}/logs/`.
