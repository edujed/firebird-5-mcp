# test-mcp — Test Database

A self-contained Firebird database for testing the `firebird-5-mcp` MCP server. All object names are in **English** to match the codegen targets.

## Why

The MCP server's codegen tools (`firebird_generate_dev`, `firebird_generate_view_dto`) and introspection tools need a database with:

- Multiple tables with FK relationships (for `@ManyToOne`, FK comments, etc.)
- All Firebird data types (INTEGER, VARCHAR, NUMERIC, TIMESTAMP, DATE, BOOLEAN, etc.)
- Domains, sequences, triggers, procedures, functions, and views
- Realistic sample data for query testing

This database provides all of that in a single, reproducible setup.

## Files

| File | Description |
|------|-------------|
| `01_drop_schema.sql` | Idempotent drop — removes all objects (triggers, procs, functions, sequences, tables, views, domains) in dependency order. |
| `02_create_schema.sql` | DDL — domains, sequences, tables, views. |
| `03_create_triggers.sql` | Triggers, functions, and procedures (uses `SET TERM ^`). |
| `04_poulate.sql` | Sample data — branches, departments, users, profiles, permissions, screens, approvals, log entries. |
| `create_test_db.sh` | Orchestrator — creates the `.fdb` (if missing), runs all 4 scripts, verifies by listing objects and counting rows. |

## Objects

### Tables (12)

| Table | PK | FKs | Notes |
|-------|----|-----|-------|
| `BRANCH` | ID | — | 4 rows, UNIQUE code |
| `DEPARTMENT` | ID | BRANCH, DEPARTMENT (self-ref) | 10 rows, hierarchical |
| `USERS` | ID | DEPARTMENT | 10 rows, UNIQUE email |
| `PROFILE` | ID | — | 5 rows |
| `PERMISSION` | ID | — | 10 rows, UNIQUE code |
| `SCREEN` | ID | — | 8 rows |
| `USER_DEPARTMENT` | ID | USERS, DEPARTMENT | 12 rows, composite UNIQUE |
| `USER_PROFILE` | ID | USERS, PROFILE | 10 rows |
| `PROFILE_PERMISSION` | ID | PROFILE, PERMISSION | 25 rows |
| `PROFILE_SCREEN` | ID | PROFILE, SCREEN | 20 rows |
| `APPROVAL` | ID | USERS (x2: requester, approver) | 8 rows |
| `ACTION_LOG` | ID | USERS | 10 rows |

### Views (2)

| View | Description |
|------|-------------|
| `VW_USER_DEPARTMENT` | JOIN USERS + USER_DEPARTMENT + DEPARTMENT + BRANCH |
| `VW_APPROVAL_SUMMARY` | JOIN APPROVAL + USERS (requester) + USERS (approver) |

### Procedures (2)

| Procedure | Description |
|-----------|-------------|
| `SP_LIST_USERS` | Returns users filtered by branch, with department and branch info |
| `SP_COUNT_APPROVALS` | Returns count of approvals filtered by user and status |

### Functions (1)

| Function | Description |
|----------|-------------|
| `FC_CALCULATE_AGE` | Returns age in years from a birth date |

### Triggers (4)

| Trigger | Table | Timing | Event |
|---------|-------|--------|-------|
| `TG_USER_EVENT` | USERS | AFTER | INSERT/UPDATE/DELETE |
| `TG_USER_UPDATE_EVENT` | USERS | AFTER | UPDATE |
| `TG_USER_DELETE_EVENT` | USERS | AFTER | DELETE |
| `TG_APPROVAL_BEFORE_INSERT` | APPROVAL | BEFORE | INSERT |

### Sequences (2)

| Sequence | Start |
|----------|-------|
| `GEN_USER_ID` | 1 |
| `GEN_APPROVAL_ID` | 1 |

### Domains (5)

| Domain | Type |
|--------|------|
| `D_BOOLEAN` | BOOLEAN DEFAULT FALSE NOT NULL |
| `D_INTEGER` | INTEGER |
| `D_TIMESTAMP` | TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL |
| `D_VARCHAR_100` | VARCHAR(100) |
| `D_VARCHAR_200` | VARCHAR(200) |

## Usage

### 1. Build the MCP binary

```bash
cd jed-personal-mcp
cargo build --release
```

### 2. Create the test database

```bash
cd examples/test-mcp

# Optional: override paths via environment
# export MCP_BINARY=/path/to/firebird-5-mcp
# export DB_PATH=/database/test-mcp.fdb

./create_test_db.sh
```

The script:
1. Creates the `.fdb` file (if it doesn't exist) and registers it in `config.json` (`register=true`)
2. Runs `01_drop_schema.sql` (idempotent drop)
3. Runs `02_create_schema.sql` (DDL)
4. Runs `03_create_triggers.sql` (triggers, functions, procedures)
5. Runs `04_poulate.sql` (sample data)
6. Verifies by listing objects and counting rows

The MCP config is resolved by the binary itself (`~/.firebird-mcp/config.json` by default). No need to set `MCP_CONFIG`.

### 3. Test the tools

```bash
# List tables
./mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Describe a table
./mcp-call.sh firebird_describe_table '{"table_name":"USERS","database":"test-mcp"}'

# Query data
./mcp-call.sh firebird_query '{"sql":"SELECT * FROM USERS","database":"test-mcp"}'

# Generate code (Java)
./mcp-call.sh firebird_generate_dev '{"tables":["USERS","DEPARTMENT"],"language":"java","database":"test-mcp"}'

# Generate code with file-save
./mcp-call.sh firebird_generate_dev '{"tables":["USERS"],"language":"java","database":"test-mcp","outputDirPath":"/tmp/mcp-out"}'

# Generate view DTO
./mcp-call.sh firebird_generate_view_dto '{"views":["VW_USER_DEPARTMENT"],"language":"typescript","database":"test-mcp"}'

# Extract metadata
./mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}'
```

## Data Types Covered

The schema exercises the following Firebird types for codegen testing:

| Type | Example Column |
|------|---------------|
| `INTEGER` | `BRANCH.ID` |
| `VARCHAR(n)` | `USERS.NAME` (100), `USERS.EMAIL` (200) |
| `NUMERIC(14,2)` | `USERS.SALARY` |
| `TIMESTAMP` | `D_TIMESTAMP` (domain), `APPROVAL.APPROVED_AT` |
| `DATE` | `USERS.HIRED_AT`, `USER_DEPARTMENT.START_DATE` |
| `BOOLEAN` | `USERS.ACTIVE`, `BRANCH.ACTIVE`, `DEPARTMENT.ACTIVE`, etc. (native Firebird 5) |

> **Note:** `BLOB`, `DOUBLE PRECISION`, `INT128`, `DEC_FLOAT16/34` are not included in the current schema. Add columns to the test tables if you need to verify codegen for those types.

## Re-running

All 4 SQL scripts are idempotent. You can re-run `create_test_db.sh` at any time to reset the database to its initial state.
