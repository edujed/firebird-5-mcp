# test-mcp — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `examples/test-mcp`. Read this first to
> locate an object before opening the full source. Line numbers are 1-based and
> refer to the file state at the time of writing — re-verify with `grep` if they
> drift.
>
> **What this example is:** a self-contained Firebird test database for the
> `firebird-5-mcp` MCP server. Provides 12 tables, 2 views, 2 procedures, 1
> function, 4 triggers, 2 sequences, and 5 domains with realistic sample data.
> All object names are in **English** to match the codegen targets.

## File map

| File | Lines | Role |
|---|---|---|
| `README.md` | 164 | Object catalog, usage, data types covered |
| `create_test_db.sh` | 155 | Orchestrator — creates `.fdb`, runs all 4 SQL scripts, verifies |
| `01_drop_schema.sql` | 145 | Idempotent drop (triggers → procs → functions → sequences → tables → views → domains) |
| `02_create_schema.sql` | 220 | DDL — domains, sequences, 12 tables, 2 views |
| `03_create_triggers.sql` | 138 | 4 triggers, 1 function, 2 procedures (uses `SET TERM ^`) |
| `04_poulate.sql` | 348 | Sample data — DELETE all, then INSERT per table |

## `create_test_db.sh`

| Item | Lines | Summary |
|---|---|---|
| Env vars | 29–31 | `MCP_BINARY`, `DB_NAME` (default `test-mcp`), `DB_PATH` (default `/database/test-mcp.fdb`) |
| `mcp_call()` | 35–60 | Sends `initialize` + `notifications/initialized` + `tools/call` in one session; pipes through `python3` to extract `result.content[0].text` |
| Preflight | 64–68 | Checks `MCP_BINARY` is executable |
| Step 1: Create DB | 74–82 | `firebird_create_database` with `register=true`; skips if already exists |
| Step 2: Drop schema | 86–90 | Runs `01_drop_schema.sql` via `firebird_run_script` |
| Step 3: Create schema | 94–98 | Runs `02_create_schema.sql` |
| Step 4: Triggers/procs | 102–106 | Runs `03_create_triggers.sql` |
| Step 5: Populate data | 110–114 | Runs `04_poulate.sql` |
| Step 6: Verify | 118–155 | Lists tables, views, procedures, functions, sequences, domains; counts rows in all 12 tables |

## `01_drop_schema.sql`

| Section | Lines | Summary |
|---|---|---|
| Header | 1–18 | Idempotent drop; uses `EXECUTE BLOCK` + `EXECUTE STATEMENT` (no `IF EXISTS` in Firebird) |
| `SET TERM ^` | 21 | Switches delimiter |
| Drop triggers | 24–36 | `TG_USER_EVENT`, `TG_USER_UPDATE_EVENT`, `TG_USER_DELETE_EVENT`, `TG_APPROVAL_BEFORE_INSERT` |
| Drop procedures | 39–51 | `SP_LIST_USERS`, `SP_COUNT_APPROVALS` |
| Drop functions | 54–66 | `FC_CALCULATE_AGE` |
| Drop sequences | 69–81 | `GEN_USER_ID`, `GEN_APPROVAL_ID` |
| Drop tables | 84–111 | 12 tables in dependency order (dependents first: `ACTION_LOG` → `BRANCH`) |
| Drop views | 114–127 | `VW_USER_DEPARTMENT`, `VW_APPROVAL_SUMMARY` |
| Drop domains | 130–142 | `D_BOOLEAN`, `D_INTEGER`, `D_TIMESTAMP`, `D_VARCHAR_100`, `D_VARCHAR_200` |
| `SET TERM ;` | 145 | Restores delimiter |

## `02_create_schema.sql`

| Section | Lines | Summary |
|---|---|---|
| Domains | 24–28 | `D_BOOLEAN`, `D_INTEGER`, `D_TIMESTAMP`, `D_VARCHAR_100`, `D_VARCHAR_200` |
| Sequences | 34–35 | `GEN_USER_ID`, `GEN_APPROVAL_ID` (both start at 1) |
| `PROFILE` | 41–48 | 5 cols, PK |

| `PERMISSION` | 50–58 | 5 cols, PK, UNIQUE code |
| `SCREEN` | 60–67 | 5 cols, PK |
| `BRANCH` | 69–78 | 6 cols, PK, UNIQUE code, uses `D_BOOLEAN`/`D_TIMESTAMP` |
| `DEPARTMENT` | 80–94 | 9 cols, PK, UNIQUE code, FK → BRANCH, self-ref FK (PARENT_ID) |
| `USERS` | 96–110 | 10 cols, PK, UNIQUE email, FK → DEPARTMENT, `NUMERIC(14,2)` salary, `DATE` hired_at |
| `USER_DEPARTMENT` | 112–124 | 7 cols, PK, composite UNIQUE, FK → USERS + DEPARTMENT |
| `USER_PROFILE` | 126–134 | 3 cols, PK, UNIQUE, FK → USERS + PROFILE |
| `PROFILE_PERMISSION` | 136–144 | 3 cols, PK, UNIQUE, FK → PROFILE + PERMISSION |
| `PROFILE_SCREEN` | 146–154 | 3 cols, PK, UNIQUE, FK → PROFILE + SCREEN |
| `APPROVAL` | 156–167 | 7 cols, PK, 2 FKs → USERS (requester + approver) |
| `ACTION_LOG` | 169–179 | 7 cols, PK, FK → USERS |
| `VW_USER_DEPARTMENT` | 185–203 | JOIN USERS + USER_DEPARTMENT + DEPARTMENT + BRANCH |
| `VW_APPROVAL_SUMMARY` | 205–220 | JOIN APPROVAL + USERS (requester) + USERS (approver) |

## `03_create_triggers.sql`

| Section | Lines | Summary |
|---|---|---|
| `SET TERM ^` | 30 | Switches delimiter |
| `TG_USER_EVENT` | 32–38 | AFTER INSERT/UPDATE/DELETE on USERS → `POST_EVENT 'user_changed'` |
| `TG_USER_UPDATE_EVENT` | 40–46 | AFTER UPDATE on USERS → `POST_EVENT 'user_updated'` |
| `TG_USER_DELETE_EVENT` | 48–54 | AFTER DELETE on USERS → `POST_EVENT 'user_deleted'` |
| `TG_APPROVAL_BEFORE_INSERT` | 56–65 | BEFORE INSERT on APPROVAL → defaults STATUS to `'PENDING'`, CREATED_AT to `CURRENT_TIMESTAMP` |
| `FC_CALCULATE_AGE` | 71–80 | Function: `BIRTH_DATE DATE` → `INTEGER` (age in years) |
| `SP_LIST_USERS` | 86–117 | Procedure: `P_BRANCH_ID INTEGER` → cursor (ID, NAME, EMAIL, DEPT_NAME, BRANCH_CODE, ACTIVE); JOINs USERS + DEPARTMENT + BRANCH |
| `SP_COUNT_APPROVALS` | 119–135 | Procedure: `P_USER_ID`, `P_STATUS` → `CNT INTEGER`; counts APPROVAL rows |
| `SET TERM ;` | 138 | Restores delimiter |

## `04_poulate.sql`

| Section | Lines | Summary |
|---|---|---|
| DELETE all | 13–24 | Clears all 12 tables (children before parents) |
| BRANCH | 30–37 | 4 rows (HQ, North, South, East) |
| DEPARTMENT | 43–62 | 10 rows (hierarchical, 2 branches) |
| USERS | 68–87 | 10 rows (Alice Johnson, Bob Smith, etc.) |
| PROFILE | 93–102 | 5 rows (Admin, Manager, Employee, Auditor, Intern) |
| PERMISSION | 108–127 | 10 rows (USER_*, APPROVAL_*, REPORT_*, SETTINGS_*) |
| SCREEN | 133–148 | 8 rows (User List, Approval Queue, Dashboard, etc.) |
| USER_DEPARTMENT | 154–177 | 12 rows (including historical/inactive assignments) |
| USER_PROFILE | 183–202 | 10 rows |
| PROFILE_PERMISSION | 208–257 | 25 rows |
| PROFILE_SCREEN | 263–302 | 20 rows |
| APPROVAL | 308–323 | 8 rows (APPROVED, PENDING, REJECTED) |
| ACTION_LOG | 329–348 | 10 rows (CREATE, UPDATE actions) |

## Conventions

- **All scripts are idempotent** — `01_drop_schema.sql` drops everything, `04_poulate.sql` DELETEs before INSERT. Safe to re-run `create_test_db.sh` at any time.
- **Drop order respects FK dependencies** — dependents dropped before referenced objects.
- **`SET TERM ^`** used in `01` and `03` for multi-statement blocks; restored to `;` at the end.
- **Data types covered:** INTEGER, VARCHAR(n), NUMERIC(14,2), TIMESTAMP, DATE, BOOLEAN (native Firebird 5). BLOB, DOUBLE PRECISION, INT128, DEC_FLOAT not included.
- **`create_test_db.sh`** requires `python3` (for JSON parsing of MCP responses) and the `firebird-5-mcp` binary.
