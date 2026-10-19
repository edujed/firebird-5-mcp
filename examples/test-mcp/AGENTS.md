# test-mcp — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `examples/test-mcp`. Read this first to
> locate an object before opening the full source. Line numbers are 1-based and
> refer to the file state at the time of writing — re-verify with `grep` if they
> drift.
>
> **What this example is:** a self-contained Firebird test database for the
> `firebird-5-mcp` MCP server. Provides 12 tables, 6 views, 2 procedures, 3
> functions, 4 triggers, 2 sequences, and 5 domains with realistic sample data
> (100 users, 35 departments, ~350-400 user-department assignments, ~400-500
> approvals, ~1500 action log entries). All object names are in **English** to
> match the codegen targets.

## File map

| File | Lines | Role |
|---|---|---|
| `README.md` | ~170 | Object catalog, usage, data types covered |
| `create_test_db.sh` | ~170 | Orchestrator — creates `.fdb`, runs all 6 SQL scripts, verifies |
| `01_drop_schema.sql` | 145 | Idempotent drop (triggers → procs → functions → sequences → tables → views → domains) |
| `02_create_schema.sql` | 220 | DDL — domains, sequences, 12 tables, 2 views |
| `03_create_triggers.sql` | 198 | 4 triggers, 3 functions (incl. name generators), 2 procedures (uses `SET TERM ^`) |
| `04_poulate.sql` | ~130 | Static seed data — DELETE all, then INSERT per table (BRANCH, DEPARTMENT, PROFILE, PERMISSION, SCREEN, PROFILE_PERMISSION, PROFILE_SCREEN) |
| `05_poulate_2.sql` | ~240 | Procedural seed data — 4 `EXECUTE BLOCK`s using `RAND()` (USERS+USER_DEPARTMENT, USER_PROFILE, APPROVAL, ACTION_LOG) |
| `06_views.sql` | ~120 | 4 additional views with JOINs (VW_EMPLOYEES, VW_USER_ASSIGNMENTS, VW_APPROVAL_FLOW, VW_DEPT_HEADCOUNT) |

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
| Step 5: Static data | 110–114 | Runs `04_poulate.sql` |
| Step 6: Procedural data | 118–122 | Runs `05_poulate_2.sql` |
| Step 7: Create views | 126–130 | Runs `06_views.sql` |
| Step 8: Verify | 134–170 | Lists tables, views, procedures, functions, sequences, domains; counts rows in all 12 tables |

## `01_drop_schema.sql`

| Section | Lines | Summary |
|---|---|---|
| Header | 1–18 | Idempotent drop; uses `EXECUTE BLOCK` + `EXECUTE STATEMENT` (no `IF EXISTS` in Firebird) |
| `SET TERM ^` | 21 | Switches delimiter |
| Drop triggers | 24–36 | `TG_USER_EVENT`, `TG_USER_UPDATE_EVENT`, `TG_USER_DELETE_EVENT`, `TG_APPROVAL_BEFORE_INSERT` |
| Drop procedures | 39–51 | `SP_LIST_USERS`, `SP_COUNT_APPROVALS` |
| Drop functions | 54–66 | `FC_CALCULATE_AGE`, `FC_FIRST_NAME`, `FC_LAST_NAME` |
| Drop sequences | 69–81 | `GEN_USER_ID`, `GEN_APPROVAL_ID` |
| Drop tables | 84–111 | 12 tables in dependency order (dependents first: `ACTION_LOG` → `BRANCH`) |
| Drop views | 114–127 | All 6 views (`VW_USER_DEPARTMENT`, `VW_APPROVAL_SUMMARY`, `VW_EMPLOYEES`, `VW_USER_ASSIGNMENTS`, `VW_APPROVAL_FLOW`, `VW_DEPT_HEADCOUNT`) |
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
| `FC_FIRST_NAME` | 86–112 | Function: `N INTEGER` → `VARCHAR(50)` (20 first names, cyclic) |
| `FC_LAST_NAME` | 114–140 | Function: `N INTEGER` → `VARCHAR(50)` (20 last names, cyclic) |
| `SP_LIST_USERS` | 146–177 | Procedure: `P_BRANCH_ID INTEGER` → cursor (ID, NAME, EMAIL, DEPT_NAME, BRANCH_CODE, ACTIVE); JOINs USERS + DEPARTMENT + BRANCH |
| `SP_COUNT_APPROVALS` | 179–195 | Procedure: `P_USER_ID`, `P_STATUS` → `CNT INTEGER`; counts APPROVAL rows |
| `SET TERM ;` | 198 | Restores delimiter |

## `04_poulate.sql` (static data)

| Section | Summary |
|---|---|
| DELETE all | Clears all 12 tables (children before parents) |
| BRANCH | 4 rows (HQ, North, South, East) |
| DEPARTMENT | 35 rows (15 physical + 20 temporary, hierarchical) |
| PROFILE | 5 rows (Admin, Manager, Employee, Auditor, Intern) |
| PERMISSION | 10 rows (USER_*, APPROVAL_*, REPORT_*, SETTINGS_*) |
| SCREEN | 8 rows (User List, Approval Queue, Dashboard, etc.) |
| PROFILE_PERMISSION | 25 rows |
| PROFILE_SCREEN | 20 rows |

## `05_poulate_2.sql` (procedural data with RAND())

| Block | Summary |
|---|---|
| USERS + USER_DEPARTMENT | 100 users (names from `FC_FIRST_NAME`/`FC_LAST_NAME`, email with `-i` suffix for uniqueness), salary/hire date/dept via `RAND()`, 1 primary + 1-5 temporary dept assignments per user |
| USER_PROFILE | 100 rows, profile via `FLOOR(RAND() * 5)` |
| APPROVAL | ~400-500 rows (2-5 physical + 0-2 temporary per user), status via `FLOOR(RAND() * 3/4)`, dates via `DATEADD(DAY, FLOOR(RAND()*365), DATE '2024-01-01')` |
| ACTION_LOG | 1500 rows, action/entity via `FLOOR(RAND()*4/5)`, timestamps via `DATEADD(HOUR, FLOOR(RAND()*8760), TIMESTAMP '2024-01-01 00:00:00')` |

## `06_views.sql`

| View | Summary |
|---|---|
| `VW_EMPLOYEES` | JOIN USERS + DEPARTMENT + BRANCH + LEFT JOIN USER_PROFILE + PROFILE (~100 rows) |
| `VW_USER_ASSIGNMENTS` | JOIN USERS + USER_DEPARTMENT + DEPARTMENT + BRANCH (~350-400 rows) |
| `VW_APPROVAL_FLOW` | JOIN APPROVAL + USERS (requester) + USERS (approver) + LEFT JOIN DEPARTMENT x2 (~400-500 rows) |
| `VW_DEPT_HEADCOUNT` | Aggregated by department: COUNT, AVG salary, MIN/MAX hire date (~35 rows) |

## Conventions

- **All scripts are idempotent** — `01_drop_schema.sql` drops everything, `04_poulate.sql` DELETEs before INSERT. Safe to re-run `create_test_db.sh` at any time.
- **Drop order respects FK dependencies** — dependents dropped before referenced objects.
- **`SET TERM ^`** used in `01`, `03`, and `05` for multi-statement blocks; restored to `;` at the end.
- **`RAND()`** used in `05_poulate_2.sql` for procedural data generation (replaces deterministic `MOD(i, n)` patterns). Volumes vary slightly per run.
- **Data types covered:** INTEGER, VARCHAR(n), NUMERIC(14,2), TIMESTAMP, DATE, BOOLEAN (native Firebird 5). BLOB, DOUBLE PRECISION, INT128, DEC_FLOAT not included.
- **`create_test_db.sh`** requires `python3` (for JSON parsing of MCP responses) and the `firebird-5-mcp` binary.
