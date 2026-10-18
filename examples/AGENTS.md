# examples — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `examples/`. Read this first to locate a
> file before opening the full source.
>
> **What this directory is:** examples and tooling for the `firebird-5-mcp` MCP
> server. The main example is a **complete test database** (`test-mcp/`) with
> 12 tables, views, triggers, procedures, functions, sequences, domains, and
> realistic seed data — created entirely through MCP tool calls (no isql). A
> Free Pascal client (`freePascal/`) demonstrates calling the MCP server from
> a non-AI, non-IDE environment.

## Subdirectories

| Directory | AGENTS.md | Role |
|---|---|---|
| `test-mcp/` | [`test-mcp/AGENTS.md`](test-mcp/AGENTS.md) | **Complete test database** — schema DDL, triggers/procs, seed data, and an orchestrator script (`create_test_db.sh`) that builds the entire `.fdb` via MCP calls |
| `freePascal/` | [`freePascal/AGENTS.md`](freePascal/AGENTS.md) | Free Pascal MCP client — spawns the MCP binary via POSIX pipes, exercises 8 tools against the `test-mcp` database |

## Files in this directory

| File | Lines | Role |
|---|---|---|
| `mcp-call.sh` | 43 | Unix CLI helper — calls one MCP tool by name + JSON args, prints the text result. Requires `python3` |
| `mcp-call.bat` | 42 | Windows CLI helper — same as `mcp-call.sh` for `.bat`/cmd. Requires `python` |

### `mcp-call.sh`

| Item | Lines | Summary |
|---|---|---|
| `MCP_BINARY` | 22 | Default binary path (override via env; currently `~/Downloads/firebird-5-mcp-linux-amd64`) |
| Usage check | 24–33 | Prints usage + examples if fewer than 2 args |
| Request build + call | 38–43 | Builds `tools/call` JSON-RPC, pipes to MCP binary, extracts `result.content[0].text` via `python3` |

### `mcp-call.bat`

| Item | Lines | Summary |
|---|---|---|
| `MCP_BINARY` | 22 | Default binary path (override via env; currently `%~dp0bin\firebird-5-mcp-windows-amd64.exe`) |
| Usage check | 24–33 | Prints usage + examples if fewer than 2 args |
| Request build + call | 38–40 | Builds `tools/call` JSON-RPC, pipes to MCP binary, extracts text via `python` |

## Conventions

- **`mcp-call.*`** are one-shot helpers: each invocation spawns the MCP binary, sends a single `tools/call`, and exits. No session persistence.
- **`MCP_BINARY`** is configurable via environment variable in both scripts. The `config.json` is resolved by the binary itself (`~/.firebird-mcp/config.json` by default).
- **`test-mcp/`** is the canonical test database. Run `test-mcp/create_test_db.sh` to (re)build it. All SQL scripts are idempotent.
- **`freePascal/`** requires the `test-mcp` database to be set up first.
