# tests — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `tests/`. Read this first to locate a
> test before opening the full source.
>
> **What this directory is:** shell smoke tests that exercise the **deployed**
> MCP binary (`~/.local/bin/firebird-5-mcp`) over JSON-RPC (stdio). These are
> integration tests — they verify the binary works end-to-end, including
> config loading, database connections, and tool dispatch. Unit tests live in
> the crates (`cargo test`).

## File map

| File | Lines | Role |
|---|---|---|
| `README.md` | 62 | Prerequisites, running instructions, script catalog |
| `test_pagination.sh` | 16 | `firebird_query` with `max_rows`/`skip` + `firebird_explain_plan` |
| `test_stats.sh` | 15 | `firebird_crypt_state` + `firebird_database_stats` |
| `test_query_history.sh` | 17 | `firebird_query_history` — no filter, state, user, limit |
| `test_isolation.sh` | 23 | `firebird_run_script` — default, snapshot, readonly, bogus, consistency |
| `test_createdb.sh` | 18 | `firebird_create_database` — default page size, explicit, invalid |
| `test_cleanup.sh` | 10 | Removes `.fdb` files created by `test_createdb.sh` |
| `test_jsonl.sh` | 32 | `firebird_export`/`firebird_describe_file`/`firebird_import` — JSONL format (plain + zstd, create + append, invalid format) |
| `test_jsonl_cleanup.sh` | 12 | Drops the `JSONL_TEST` table created by `test_jsonl.sh` |
| `test_pagination.bat` | 18 | Windows equivalent of `test_pagination.sh` |
| `test_stats.bat` | 17 | Windows equivalent of `test_stats.sh` |
| `test_query_history.bat` | 19 | Windows equivalent of `test_query_history.sh` |
| `test_createdb.bat` | 21 | Windows equivalent of `test_createdb.sh` |
| `test_cleanup.bat` | 12 | Windows equivalent of `test_cleanup.sh` |

## How the tests work

Each `.sh` script:
1. Sets `BIN="${HOME}/.local/bin/firebird-5-mcp"` (the deployed binary)
2. Pipes a sequence of JSON-RPC messages (initialize → notifications/initialized → tools/call) to the binary
3. Uses `grep` to filter the response lines by `id`
4. Prints the raw JSON — look for `"isError":false` and the expected text

**No `FIREBIRD_CONFIG` env var** — the binary uses its default config path
(`~/.firebird-mcp/config.json`).

## Running

```bash
# Linux/macOS
sh tests/test_pagination.sh
sh tests/test_stats.sh
sh tests/test_query_history.sh
sh tests/test_isolation.sh
sh tests/test_createdb.sh     # creates real .fdb files
sh tests/test_cleanup.sh      # removes the files from test_createdb.sh
sh tests/test_jsonl.sh        # requires the test-mcp database
sh tests/test_jsonl_cleanup.sh # drops the JSONL_TEST table

# Windows
tests\test_pagination.bat
tests\test_stats.bat
tests\test_query_history.bat
tests\test_createdb.bat       :: creates real .fdb files
tests\test_cleanup.bat        :: removes the files from test_createdb.bat
```

> **Note:** `test_isolation.bat` and `test_jsonl*.sh` do not exist yet (Linux-only for now).

## Prerequisites

- Binary built and installed: `./scripts/build.sh` → `~/.local/bin/firebird-5-mcp`
- Config at `~/.firebird-mcp/config.json` with at least one database
- The database must exist and be accessible (e.g. `/database/teste.fdb`)

## Conventions

- **Smoke tests, not unit tests** — they verify the deployed binary works end-to-end.
- **`test_createdb` writes to the server's allowed path** (`/database` on Linux). Always follow it with `test_cleanup`.
- **`test_jsonl` writes to `~/.firebird-mcp/output/` and creates a table in `test-mcp`.** Always follow it with `test_jsonl_cleanup`.
- **`.sh` scripts** use `#!/bin/sh` (POSIX), not bash. No arrays, no `[[ ]]`.
- **`.bat` scripts** use `%USERPROFILE%\.local\bin\firebird-5-mcp.exe` (same as `.sh` but Windows path). No `FIREBIRD_CONFIG` — the binary uses its default config.
- **Expected failures:** `test_isolation.sh` id=4 (readonly) and id=5 (bogus) should return `"isError":true`. `test_createdb.sh` id=4 (invalid page size) should return `"isError":true`.
