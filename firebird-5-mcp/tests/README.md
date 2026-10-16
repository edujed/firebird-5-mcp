# Shell tests for the Firebird 5 MCP

These scripts exercise the **deployed** MCP binary over JSON-RPC (stdio),
which is useful for verifying new tools *before* restarting Zed (Zed only
picks up a new binary on restart).

## Prerequisites

- The release binary built and deployed:
  ```sh
  cd firebird-5-mcp
  cargo build --release
  cp target/release/firebird-5-mcp \
     ../bin/firebird-5-mcp-linux-amd64.new
  mv -f ../bin/firebird-5-mcp-linux-amd64.new \
        ../bin/firebird-5-mcp-linux-amd64
  ```
- A `config.json` at `../config.json` (the scripts set
  `FIREBIRD_CONFIG` to that path).

## Running

**Linux / macOS** (`.sh`):

```sh
cd firebird-5-mcp
sh tests/test_pagination.sh
sh tests/test_stats.sh
sh tests/test_query_history.sh
sh tests/test_createdb.sh     # creates real .fdb files
sh tests/test_cleanup.sh      # removes the files from test_createdb.sh
```

**Windows** (`.bat`):

```bat
cd firebird-5-mcp
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
| `test_createdb.{sh,bat}` | `firebird_create_database` with default page size, explicit `page_size`, and an invalid value |
| `test_cleanup.{sh,bat}` | Deletes the `.fdb` files created by `test_createdb` |

## Notes

- The `.sh` scripts hard-code the Linux binary path
  (`/home/edujed/github/plamenix/jed-personal-mcp/bin/firebird-5-mcp-linux-amd64`)
  and the config path. The `.bat` scripts use relative paths
  (`..\..\bin\firebird-5-mcp-windows-amd64.exe` and `..\..\config.json`).
  Adjust them if your layout differs.
- `test_createdb` writes to the server's allowed path (`/database` on Linux,
  `C:\database` on Windows). Always follow it with `test_cleanup`.
- These are smoke tests, not unit tests. Unit tests live in the crates
  (`cargo test`).
