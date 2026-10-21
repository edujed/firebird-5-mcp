# firebird-5-mcp — For Humans

> **You don't need an AI agent to use this tool.**
>
> `firebird-5-mcp` is an MCP server for Firebird — but it's also a **complete CLI**
> that runs in cron jobs, CI pipelines, shell scripts, and anywhere else you'd
> script a database. No IDE. No LLM. No agent. Just a single static binary
> that speaks JSON-RPC over stdin/stdout.

Think of it as **Firebird PowerToys**: the same binary that powers AI agents
also gives you a Swiss-army knife for everyday database work — exporting
tables to your DataLake, consolidating data from other tools, running batch
procedures on a schedule, snapshotting your schema for version control.

---

## Why use it without AI?

| Scenario | What you get |
|---|---|
| **DataLake / BI** | Export any table, view, or SQL query to Parquet, CSV, DBF, or JSON Lines — one command, cron-ready |
| **Data consolidation** | Import Parquet/CSV/DBF/JSONL from other tools into Firebird — create or append, skip columns |
| **Batch jobs** | Run stored procedures or SQL scripts on a schedule (month-end close, nightly ETL) |
| **Health checks** | Database stats, active queries, table row counts — pipe to your monitoring/alerting |
| **Schema versioning** | Extract full DDL, diff it in git, track every change |
| **CI pipelines** | Run test scripts against a scratch database, verify row counts, tear down |
| **Quick introspection** | Describe a table, find a column, check for duplicates — without opening a GUI |

---

## How it works

The binary speaks **JSON-RPC 2.0** over stdin/stdout:

1. You pipe a JSON-RPC request to the binary's **stdin**
2. It processes the tool call and writes the response to **stdout**
3. When stdin reaches **EOF**, the binary exits

This makes it perfect for pipes, `xargs`, `cron`, and any shell workflow:

```
echo '<json-rpc>' | firebird-5-mcp | jq '.result.content[0].text'
```

One line in, one line out. No daemon. No port. No state.

---

## Quick start

### 1. Get the binary

**Option A — GitHub Releases (most users):**

1. Go to [Releases](https://github.com/your-org/firebird-5-mcp/releases)
2. Download the `.tgz` for your platform (e.g. `firebird-5-mcp-linux-amd64.tgz`)
3. Extract: `tar xzf firebird-5-mcp-linux-amd64.tgz`
4. You now have the `firebird-5-mcp` binary

**Option B — Build from source (developers):**

```bash
git clone https://github.com/your-org/firebird-5-mcp.git
cd firebird-5-mcp
./scripts/build.sh          # compiles → ~/.local/bin/firebird-5-mcp
```

### 2. Configure

The binary auto-loads `~/.firebird-mcp/config.json` (or `FIREBIRD_CONFIG` env,
or `./config.json`). Minimum config:

```json
{
  "server": {
    "host": "localhost",
    "port": 3050,
    "user": "SYSDBA",
    "password": "masterkey",
    "allowed_paths": ["/database"],
    "databases": [
      {
        "name": "test-mcp",
        "description": "MCP test database",
        "path": "/database/test-mcp.fdb"
      }
    ],
    "default_database": "test-mcp"
  }
}
```

> **Tip:** the [`examples/test-mcp/`](examples/test-mcp/) directory contains a
> complete test database (18 tables, 12 procedures, 10 functions, views,
> triggers) with a one-shot creation script:
>
> ```bash
> cd examples/test-mcp
> ./create_test_db.sh
> ```

### 3. Make your first call

The [`examples/mcp-call.sh`](examples/mcp-call.sh) helper wraps the JSON-RPC
protocol so you can call any tool with one command:

```bash
# List all tables
./examples/mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Query data
./examples/mcp-call.sh firebird_query '{"sql":"SELECT * FROM USERS","database":"test-mcp"}'

# Describe a table
./examples/mcp-call.sh firebird_describe_table '{"table_name":"USERS","database":"test-mcp"}'

# Count rows
./examples/mcp-call.sh firebird_count '{"table":"USERS","database":"test-mcp"}'
```

> **Note:** set `MCP_BINARY` in the script (or as an environment variable) to
> point to your binary. The config file is resolved automatically.

---

## Practical recipes

All examples below use the `test-mcp` database.

### Export a table to your DataLake (Parquet)

```bash
# Export USERS to Parquet (Snappy-compressed)
./examples/mcp-call.sh firebird_export '{
  "source": "USERS",
  "database": "test-mcp",
  "output_path": "/datalake/firebird/users.parquet",
  "format": "parquet",
  "compression": "snappy"
}'
```

**Cron job** — nightly export of all tables:

```bash
# /etc/cron.d/firebird-export
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"USERS","database":"test-mcp","output_path":"/datalake/firebird/users.parquet","format":"parquet"}'
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"DEPARTMENT","database":"test-mcp","output_path":"/datalake/firebird/department.parquet","format":"parquet"}'
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"APPROVAL","database":"test-mcp","output_path":"/datalake/firebird/approval.parquet","format":"parquet"}'
```

You can also export a **SQL query** (not just a table):

```bash
# Export a joined view
./examples/mcp-call.sh firebird_export '{
  "source": "SELECT U.NAME, D.NAME AS DEPT, B.CODE AS BRANCH FROM USERS U JOIN DEPARTMENT D ON D.ID = U.DEPARTMENT_ID JOIN BRANCH B ON B.ID = D.BRANCH_ID",
  "database": "test-mcp",
  "output_path": "/datalake/firebird/users_with_dept.csv",
  "format": "csv"
}'
```

### Import data from another tool

```bash
# Import a CSV (table must already exist)
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/sales_report.csv",
  "database": "test-mcp",
  "table": "SALES",
  "mode": "append",
  "format": "csv"
}'

# Import a Parquet file, creating the table automatically
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/metrics.parquet",
  "database": "test-mcp",
  "table": "METRICS",
  "mode": "create",
  "format": "parquet"
}'

# Skip columns you don't want
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/legacy.dbf",
  "database": "test-mcp",
  "table": "LEGACY_DATA",
  "mode": "create",
  "format": "dbf",
  "ignore_columns": ["_DELETED", "FILLER"]
}'

# Import JSON Lines (one JSON object per line) — types are inferred automatically
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/events.jsonl.zst",
  "database": "test-mcp",
  "table": "EVENTS",
  "mode": "create",
  "format": "jsonl"
}'
```

### Run a stored procedure in a batch job

The `test-mcp` database includes `SP_COUNT_APPROVALS` (counts approvals by
user/status) and `SP_LIST_USERS` (lists users with department/branch info).

```bash
# Count pending approvals
./examples/mcp-call.sh firebird_query '{
  "sql": "SELECT * FROM RDB$PROCEDURES WHERE RDB$PROCEDURE_NAME = ''SP_COUNT_APPROVALS''",
  "database": "test-mcp"
}'

# Execute a procedure and get results
./examples/mcp-call.sh firebird_query '{
  "sql": "select * from SP_COUNT_APPROVALS(1, ''PENDING'')",
  "database": "test-mcp"
}'
```

**Cron job** — run a month-end close procedure:

```bash
# /etc/cron.d/firebird-month-end
0 6 1 * * root /path/to/mcp-call.sh firebird_query '{"sql":"EXECUTE PROCEDURE SP_MONTH_END_CLOSE()","database":"test-mcp"}'
```

Or run a full SQL script:

```bash
# Run a batch script file
./examples/mcp-call.sh firebird_run_script '{
  "database": "test-mcp",
  "path": "/scripts/month_end_close.sql"
}'
```

### Health check + alerting

```bash
# Database stats (size, connections, I/O, active statements)
./examples/mcp-call.sh firebird_database_stats '{"database":"test-mcp"}'

# Active queries (who's running what right now)
./examples/mcp-call.sh firebird_query_history '{"database":"test-mcp","state":1}'

# Row counts for all tables
./examples/mcp-call.sh firebird_table_stats '{"database":"test-mcp"}'
```

**Cron job** — daily health check with alert:

```bash
#!/bin/bash
# /usr/local/bin/firebird-healthcheck.sh
OUTPUT=$(./examples/mcp-call.sh firebird_database_stats '{"database":"test-mcp"}')
if echo "$OUTPUT" | grep -q "Read-Only: YES"; then
    echo "ALERT: test-mcp is READ-ONLY!" | mail -s "Firebird Alert" dba@example.com
fi
```

### Schema snapshot for version control

```bash
# Extract full DDL (replaces isql -x)
./examples/mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}' > schema.sql

# Commit to git
git add schema.sql
git diff --staged schema.sql   # review changes
git commit -m "schema: add APPROVED_AT to APPROVAL"
```

**Cron job** — nightly schema diff:

```bash
#!/bin/bash
# /usr/local/bin/firebird-schema-diff.sh
cd /repo/schema
./examples/mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}' > current.sql
if ! git diff --quiet current.sql; then
    echo "Schema changed:" | mail -s "Firebird Schema Change" dba@example.com
    git diff current.sql | mail -s "Firebird Schema Diff" dba@example.com
fi
```

### Generate code from your schema

```bash
# Generate TypeScript interfaces
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS", "DEPARTMENT"],
  "language": "typescript",
  "database": "test-mcp"
}'

# Generate OpenAPI spec
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS"],
  "language": "openapi",
  "database": "test-mcp"
}'

# Generate Java entities (JPA + Lombok)
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS", "APPROVAL"],
  "language": "java",
  "database": "test-mcp",
  "outputDirPath": "/tmp/generated"
}'
```

---

## The power table

| Tool | Human use case |
|---|---|
| `firebird_export` | Publish a table/view/query to your DataLake (Parquet/CSV/DBF/JSONL) |
| `firebird_import` | Consolidate data from other tools into Firebird |
| `firebird_describe_file` | Inspect an incoming file's schema before importing |
| `firebird_query` | Run any SQL — SELECT, DML, DDL, stored procedures |
| `firebird_run_script` | Execute a multi-statement SQL script (batch jobs, migrations) |
| `firebird_insert_batch` | Bulk-load rows in a single transaction |
| `firebird_database_stats` | Health check — size, connections, I/O, active statements |
| `firebird_query_history` | See who's running what (active/idle/stalled queries) |
| `firebird_table_stats` | Row counts per table (monitoring, capacity planning) |
| `firebird_metadata_extract` | Full DDL snapshot for git versioning |
| `firebird_describe_database` | Complete schema overview + Mermaid ER diagram |
| `firebird_describe_table` | Full schema of one table (columns, indexes, constraints, triggers) |
| `firebird_list` | List tables, views, procedures, functions, triggers, indexes, sequences |
| `firebird_find_column` | Find a column by name pattern across all tables |
| `firebird_check_duplicates` | Verify data integrity before creating a unique constraint |
| `firebird_count` | Quick row count (with optional WHERE) |
| `firebird_sample` | Quick peek at table data |
| `firebird_explain_plan` | Inspect a query's execution plan (without running it) |
| `firebird_generate_dev` | Generate code (Java, Go, Rust, TS, Protobuf, OpenAPI, Lazarus) |
| `firebird_generate_view_dto` | Generate DTOs from views |
| `firebird_create_database` | Create a new .fdb file |
| `firebird_create_trigger` | Create a trigger (BEFORE/AFTER INSERT/UPDATE/DELETE) |
| `firebird_create_procedure` | Create a stored procedure |
| `firebird_create_function` | Create a user-defined function |
| `firebird_create_sequence` | Create a sequence (generator) |
| `firebird_alter_column_type` | Change a column's type using a domain |
| `firebird_drop` | Drop a table, view, trigger, procedure, function, index, sequence |
| `firebird_users` | List all users with database access |
| `firebird_grants` | Show GRANTs for a table or all tables |
| `firebird_blob_info` | List BLOB columns with subtype |
| `firebird_index_usage` | Show all indexes with columns and uniqueness |
| `firebird_domains` | Analyze user-defined domains and usage |
| `firebird_get_databases` | List configured databases |
| `firebird_crypt_state` | Check at-rest encryption state |
| `firebird_data_quality` | Rule-based data quality checks (nulls, empty strings, outliers, date range) |
| `firebird_list_files` | List generated files in the storage directory |

---

## References

| Document | What it covers |
|---|---|
| [`README.md`](README.md) | Full tool reference (all 36 tools with parameters and examples) |
| [`README-PTBR.md`](README-PTBR.md) | Same, in Portuguese |
| [`README_COMMAND_LINE.md`](README_COMMAND_LINE.md) | JSON-RPC protocol details, helper scripts |
| [`README_WEB_UI.md`](README_WEB_UI.md) | Web UI / non-stdio clients (HTTP+SSE transport) |
| [`examples/test-mcp/`](examples/test-mcp/) | Complete test database (schema, seed data, creation script) |
| [`examples/mcp-call.sh`](examples/mcp-call.sh) | CLI helper (Linux/macOS) |
| [`examples/mcp-call.bat`](examples/mcp-call.bat) | CLI helper (Windows) |

---

## License

See [LICENSE](LICENSE).
