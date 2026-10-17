# Shell tests for the Firebird 5 MCP

These scripts exercise the **deployed** MCP binary over JSON-RPC (stdio),
which is useful for verifying new tools *before* restarting Zed (Zed only
picks up a new binary on restart).

## Prerequisites

- The release binary built and installed:
  ```sh
  ./scripts/build.sh
  ```
  This installs the binary to `~/.local/bin/firebird-5-mcp`.
- A `config.json` at `~/.firebird-mcp/config.json` (the default path).
  The scripts do **not** set `FIREBIRD_CONFIG` — the binary uses its default.

## Running

**Linux / macOS** (`.sh`):

```sh
sh tests/test_pagination.sh
sh tests/test_stats.sh
sh tests/test_query_history.sh
sh tests/test_isolation.sh
sh tests/test_createdb.sh     # creates real .fdb files
sh tests/test_cleanup.sh      # removes the files from test_createdb.sh
```

**Windows** (`.bat`):

```bat
tests\test_pagination.bat
tests\test_stats.bat
tests\test_query_history.bat
tests\test_createdb.bat       :: creates real .fdb files
tests\test_cleanup.bat        :: removes the files from test_createdb.bat
```

Each script prints the raw JSON-RPC responses. Look for `"isError":false`
and the expected text in the `content` field.

## Scripts

| Script | What it tests |
|--------|---------------|
| `test_pagination.{sh,bat}` | `firebird_query` with `max_rows`/`skip` (pagination hint) and `firebird_explain_plan` with single quotes (injection fix) |
| `test_stats.{sh,bat}` | `firebird_crypt_state` and `firebird_database_stats` |
| `test_query_history.{sh,bat}` | `firebird_query_history` with no filter, `state` filter, `user` filter, and `limit` |
| `test_isolation.sh` | `firebird_run_script` with default, snapshot, readonly, bogus, and consistency isolation |
| `test_createdb.{sh,bat}` | `firebird_create_database` with default page size, explicit `page_size`, and an invalid value |
| `test_cleanup.{sh,bat}` | Deletes the `.fdb` files created by `test_createdb` |

## Notes

- The `.sh` scripts use `~/.local/bin/firebird-5-mcp` (the path installed by
  `scripts/build.sh`). The `.bat` scripts use `%USERPROFILE%\.local\bin\firebird-5-mcp.exe`.
  No `FIREBIRD_CONFIG` env var — the binary uses its default config path.
- `test_createdb` writes to the server's allowed path (`/database` on Linux,
  `C:\database` on Windows). Always follow it with `test_cleanup`.
- These are smoke tests, not unit tests. Unit tests live in the crates
  (`cargo test`).
