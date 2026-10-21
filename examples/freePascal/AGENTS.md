# freePascal — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `examples/freePascal`. Read this first to
> locate a function before opening the full source. Line numbers are 1-based and
> refer to the file state at the time of writing — re-verify with `grep` if they
> drift.
>
> **What this example is:** a Free Pascal command-line MCP client that spawns the
> `firebird-5-mcp` binary via POSIX pipes (`fork`/`exec`/`read`/`write`), sends
> JSON-RPC requests, and parses the response with `fpjson`. No IDE or AI
> integration required.

## File map

| File | Lines | Role |
|---|---|---|
| `README.md` | 256 | Build/run instructions, expected output, architecture diagram |
| `uMCP.pas` | 243 | MCP client unit — POSIX pipe wrappers + `MCPCall` / `MCPListTools` |
| `test_mcp.pas` | 67 | Test program exercising 8 MCP tools against the `test-mcp` database |
| `test_events.pas` | 258 | Test program for the universal POST_EVENT feature (`on_success_event` / `on_fail_event`) — verifies `events.jsonl` lines and Firebird `SET EVENT` notifications |
| `test_migration.pas` | 70 | Test program for the migration tools (`firebird_export` / `firebird_describe_file` / `firebird_import`) |

## `uMCP.pas`

| Item | Lines | Summary |
|---|---|---|
| `MCP_BINARY` const | 20 | Path to the `firebird-5-mcp` binary (edit this; should point to `~/.local/bin/firebird-5-mcp`) |
| `MCPCall` (interface) | 23 | Public: calls an MCP tool, returns text result |
| `MCPListTools` (interface) | 26 | Public: lists all available MCP tools |
| `TFDArray` type | 34 | `array[0..1] of Integer` for pipe file descriptors |
| POSIX externals | 42–49 | `pipe`, `fork`, `execv`, `dup2`, `close`, `read`, `write`, `waitpid` (declared `external 'libc'`) |
| `ExecuteMCP()` | 52–137 | Core: creates pipes, forks, child `execv`s the MCP binary, parent writes JSON-RPC to stdin, reads all stdout, `waitpid` |
| `MCPCall()` | 139–184 | Builds `tools/call` JSON-RPC, calls `ExecuteMCP`, parses `result.content[0].text` via `fpjson` |
| `MCPListTools()` | 186–241 | Builds `tools/list` JSON-RPC, calls `ExecuteMCP`, parses `result.tools[]` into a formatted list |

## `test_mcp.pas`

| Item | Lines | Summary |
|---|---|---|
| `PrintSection()` | 20–26 | Prints a `====` section header |
| `begin` block | 28–67 | Runs 8 tests: `tools/list`, `firebird_list` (TABLES), `firebird_describe_table` (USERS), `firebird_query` (SELECT), `firebird_count` (USERS), `firebird_generate_dev` (Lazarus), `firebird_generate_dev` (OpenAPI), `firebird_generate_dev` (Java + `outputDirPath`) |

## Conventions

- **No `TProcess`** — FPC 3.2.2's `TProcess` lacks `WaitFor`/`OutputLines`/`CloseInput`. Raw POSIX calls via `external 'libc'` instead.
- **One process per call** — each `MCPCall` spawns a new MCP binary. The server exits on stdin EOF, which is what makes the pipe approach work.
- **`MCP_BINARY` is hardcoded** in `uMCP.pas` (L20). Should point to `~/.local/bin/firebird-5-mcp` (the path used by `scripts/build.sh`). For production, make it configurable.
- **Build:** `fpc -otest_mcp test_mcp.pas` (compiles both `.pas` files into one static binary).
- **Requires:** FPC 3.2.2+, `fcl-json` package, `firebird-5-mcp` binary, `config.json` with `test-mcp` database.
