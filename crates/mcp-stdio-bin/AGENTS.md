# mcp-stdio-bin — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-stdio-bin`. Read this first to locate a
> function before opening the full source. Line numbers are 1-based and refer to
> the file state at the time of writing — re-verify with `grep` if they drift.
>
> **What this crate is:** the MCP server binary (`firebird-5-mcp`). It supports
> **two transports**, selected via CLI flags:
>
> - **stdio** (default) — JSON-RPC 2.0, line-delimited, over stdin/stdout.
> - **SSE** (`--sse`) — HTTP+SSE (spec `2024-11-05`), for web UIs and
>   non-stdio clients (llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode).
>
> All tool handlers and the protocol routing logic live in the sibling crate
> `firebird-5-mcp-tools` (`crates/mcp-tools`) — this crate is a thin transport
> layer + CLI flag parsing + auth.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | ~18 | Package `firebird-5-mcp`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-tools`, `serde`, `serde_json`, `tokio`, `axum` (multipart), `uuid`, `async-stream` |
| `src/main.rs` | 311 | Entry point, CLI flag parsing, transport dispatch (stdio vs. SSE), config loading, logging init |
| `src/sse.rs` | 354 | MCP HTTP+SSE transport (spec `2024-11-05`) — `GET /sse` + `POST /messages`, session management (UUID → broadcast channel), background cleanup, CORS support. Merges the files router in `run_sse()` |
| `src/files.rs` | 357 | File upload/download endpoints — `GET /files?path=<rel>` (download) + `POST /files` (multipart upload, 500 MB limit). Path-traversal protection, same auth as SSE, CORS support |
| `src/auth.rs` | ~160 | Bearer-token authentication (disabled by default) — `AuthConfig`, `check_auth()`, constant-time comparison, 5 unit tests |

## `src/main.rs`

| Function | Lines | Summary |
|---|---|---|
| `struct Args` | 26–40 | CLI args: `sse`, `host`, `port`, `token`, `token_file`, `session_timeout_min`, `base_url` |
| `parse_args()` | 55–131 | Parses `--sse`, `--host`, `--port`, `--token`, `--token-file`, `--session-timeout`, `--base-url`, `--help`. Returns `Result<Args, String>` |
| `print_usage()` | 132–158 | Usage message with examples |
| `main()` | 159–253 | Loads `Config`, inits logging, computes `files_base_url` (SSE only: `--base-url` or derived from `--host`/`--port`, `None` for wildcard hosts), creates `ProtocolCore::new(cfg, files_base_url)`, dispatches to `run_sse()` (if `--sse`) or `run_stdio()` |
| `resolve_auth()` | 254–279 | Resolves `AuthConfig` from args: `--token-file` > `--token` > disabled |
| `run_stdio()` | 280–311 | Stdio transport: reads JSON-RPC lines from stdin, calls `core.handle_request()`, writes to stdout |

## `src/sse.rs`

| Function | Lines | Summary |
|---|---|---|
| `struct Session` | 43–55 | A single SSE session: `broadcast::Sender<Value>` + `last_active` timestamp. `new()` creates the channel (capacity 64) |
| `struct SseState` | 66–74 | Shared state: `Arc<ProtocolCore>`, `Arc<Mutex<HashMap<Uuid, Session>>>`, `Arc<AuthConfig>`. `Clone` (all fields are `Arc`) |
| `struct MessageQuery` | 77–80 | Query params for `POST /messages`: `sessionId` (renamed from `session_id` via `#[serde(rename)]`) |
| `build_router()` | 87–108 | Builds the axum `Router` with `GET /sse` and `POST /messages`, attaches `Arc<SseState>` via `.with_state()` |
| `add_cors_headers()` | 141–150 | Appends `Access-Control-Allow-*` headers to a response (shared by all SSE handlers) |
| `handle_sse()` | 152–205 | `GET /sse` handler. Authenticates, creates a session (UUID + broadcast channel), returns an `Sse` stream: first event is `endpoint` (with the POST URL), then bridges the broadcast channel to `message` events. Keep-alive every 15s |
| `handle_message()` | 207–294 | `POST /messages` handler. Authenticates, parses `sessionId` + JSON body, looks up the session, runs `core.handle_request()` in `spawn_blocking` (avoids stalling the async runtime), sends the response via the broadcast channel. Returns `202 Accepted` |
| `spawn_session_cleanup()` | 296–330 | Background task: reaps sessions inactive for > timeout (default 30 min). Runs every 60s |
| `run_sse()` | 332–354 | Entry point for SSE mode: creates the router, spawns the cleanup task, binds the TCP listener, runs `axum::serve()` |

## `src/auth.rs`

| Function | Lines | Summary |
|---|---|---|
| `struct AuthConfig` | ~25–50 | `token: Option<String>`. `None` = auth disabled. `with_token()`, `disabled()`, `is_enabled()` |
| `check_auth()` | ~60–90 | Validates the `Authorization: Bearer <token>` header. Returns `Ok(())` if valid (or auth disabled), `Err(Response)` with `401` otherwise. Uses constant-time comparison |
| `unauthorized()` | ~95–105 | Builds a `401 Unauthorized` response with a JSON-RPC error body |
| `constant_time_eq()` | ~110–120 | Constant-time byte comparison (mitigates timing attacks) |
| `mod tests` | ~125–160 | 5 unit tests: disabled allows all, valid token, invalid token, missing header, constant-time eq |

## What lives in `mcp-tools` (sibling crate)

All tool handlers, `ToolResult`, `dispatch()`, `tool_definitions()`, and
`ProtocolCore` live in `crates/mcp-tools/`. See that crate's `AGENTS.md` for
the full handler map.

## `src/files.rs`

| Function | Lines | Summary |
|---|---|---|
| `const MAX_UPLOAD_BYTES` | 28 | Upload size limit: 500 MB |
| `struct FileQuery` | 32–35 | Query params for `GET /files`: `path` (relative path in storage dir) |
| `struct FilesState` | 39–42 | Shared state: `Arc<ProtocolCore>` + `Arc<AuthConfig>`. `Clone` |
| `build_files_router()` | 45–62 | Builds the axum `Router` with `GET /files` and `POST /files` on the same path |
| `add_cors_headers()` | 90–99 | Appends `Access-Control-Allow-*` headers to a response (shared by all file handlers) |
| `resolve_storage_dir()` | 100–113 | Resolves the storage base dir from live config (`config.storage.dir` > `~/.firebird-mcp/output/`) |
| `resolve_safe_path()` | 115–174 | Resolves a relative path against the storage base, rejecting path traversal (`..`, absolute paths, symlink escape). Handles non-existent paths (for uploads) by canonicalizing the deepest existing ancestor |
| `handle_download()` | 176–230 | `GET /files?path=<rel>` — reads the file, returns it with `Content-Disposition: attachment` |
| `handle_upload()` | 232–329 | `POST /files` — multipart with `file` (binary) + `target` (relative path). Creates parent dirs, enforces 500 MB limit |
| Error helpers | 331–357 | `bad_request()`, `not_found()`, `too_large()`, `internal_error()` — JSON error responses |

## Conventions

- This crate contains **no tool handlers** — they live in `mcp-tools`.
- The protocol routing logic (`ProtocolCore`) lives in `mcp-tools` — this
  crate only reads requests from the transport and writes responses back.
- Config is loaded once at startup and stored in `ProtocolCore` (which wraps
  it in a `RwLock`). `firebird_get_databases` can reload it;
  `firebird_create_database` can update it in-memory.
- Logging goes through `mlog!` (core) to `{base}/logs/`.
- **SSE sessions** are UUID-keyed broadcast channels. Each `GET /sse` creates
  a new session; the `POST /messages` handler looks up the session by UUID and
  sends the response through the channel. Sessions are reaped after 30 min of
  inactivity (configurable via `--session-timeout`).
- **Concurrency:** each tool call runs in `spawn_blocking` (tokio thread
  pool), each opening its own Firebird connection. No global mutex — safe for
  20+ simultaneous users.
- **Auth** is disabled by default. Enable with `--token` or `--token-file`.
  Token expiration/rotation is delegated to an external auth interface.
- **File endpoints** (`GET /files`, `POST /files`) are only available in SSE mode.
  They use the same bearer-token auth as the SSE endpoints. The storage directory
  is resolved from the live config (same priority as the tools). Path traversal
  is blocked (rejects `..`, absolute paths, and symlink escapes). Upload limit:
  500 MB. The `firebird_list_files` tool returns relative paths that can be used
  directly as the `path` query parameter for downloads.
- **`download_url` in tool responses (SSE only):** in SSE mode, `main()` computes
  a `files_base_url` (`--base-url` if given, else `http://<host>:<port>` derived
  from `--host`/`--port`; `None` when the bind host is a wildcard like `0.0.0.0`
  or `::`, since that is not a usable URL host) and passes it to
  `ProtocolCore::new`. File-generating tools then include a `download_url`
  (`{base}/files?path=<rel>`) in their structured response so clients can
  retrieve the file. In stdio mode the base URL is `None` and only the absolute
  `path` is returned. Use `--base-url http://<lan-ip>:<port>` to advertise a
  LAN-reachable host when binding to `0.0.0.0`.
- **CORS:** all SSE endpoints (`/sse`, `/messages`, `/files`) support CORS for
  browser clients. Preflight `OPTIONS` requests return 204 with
  `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Methods: GET, POST,
  OPTIONS`, and `Access-Control-Allow-Headers: Authorization, Content-Type`.
  Regular responses include `Access-Control-Allow-Origin: *`. This allows web UIs
  hosted on different origins to communicate with the MCP server.
