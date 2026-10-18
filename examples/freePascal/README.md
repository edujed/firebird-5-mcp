# Free Pascal / Lazarus — MCP Client Example

Example of calling the `firebird-5-mcp` MCP server from a Free Pascal command-line program, without any IDE or AI integration.

For IDEs without MCP support (like Lazarus), you can call the MCP server directly from Pascal code. The examples use **raw POSIX pipes** (`fork`/`exec`/`read`/`write`) instead of `TProcess`, because the `TProcess` API in FPC 3.2.2 is limited (no `WaitFor`, `OutputLines`, or `CloseInput`).

> **Note:** Each `MCPCall` spawns a new process. For high-frequency calls, consider keeping a persistent connection (write to stdin, read from stdout in a thread). For typical dev workflows, the spawn-per-call approach is simple and sufficient.

## Files

| File | Description |
|------|-------------|
| `uMCP.pas` | MCP client unit — spawns the MCP binary via POSIX pipes (`fork`/`exec`/`read`/`write`), sends JSON-RPC requests, and parses the response with `fpjson`. |
| `test_mcp.pas` | Test program that exercises 8 MCP tools against the `test-mcp` database: `tools/list`, `firebird_list`, `firebird_describe_table`, `firebird_query`, `firebird_count`, `firebird_generate_dev` (Lazarus), `firebird_generate_dev` (OpenAPI), and `firebird_generate_dev` (Java with `outputDirPath`). |
| `test_migration.pas` | Migration tools example — exports `USERS` to Parquet, describes the file, imports it into a new table, verifies, and cleans up. |

## Requirements

- **Free Pascal** 3.2.2+ (tested on FPC 3.2.2 / Linux x86_64)
- **`fcl-json`** package (provides `fpjson` and `jsonparser` units) — included in standard FPC distributions
- **`firebird-5-mcp`** binary (built from the Rust project)
- **`config.json`** with the `test-mcp` database configured (see `../test-mcp/README.md`)

## Configuration

Edit the constant at the top of `uMCP.pas`:

```pascal
const
  MCP_BINARY  = '/path/to/firebird-5-mcp';
```

The MCP config is resolved by the binary itself (`~/.firebird-mcp/config.json` by default). No need to set a config path.

## Build

```bash
# Main test (8 tools)
fpc -otest_mcp test_mcp.pas

# Migration tools example (Parquet export/import)
fpc -otest_migration test_migration.pas
```

This compiles the program and `uMCP.pas` (found in the same directory) into a single static binary.

## Run

```bash
./test_mcp
./test_migration
```

Expected output:

```
firebird-5-mcp — Free Pascal MCP Client Test
============================================

========================================
1. Available Tools (tools/list)
========================================
33 tools available:
  firebird_query — Executes a single SQL statement...
  firebird_describe_table — Shows the full schema...
  ...

========================================
2. List Tables (firebird_list)
========================================
### Tables and Views (12)

NAME | TYPE | COMMENT
--- | --- | ---
ACTION_LOG | TABLE | -
APPROVAL | TABLE | -
BRANCH | TABLE | -
DEPARTMENT | TABLE | -
PERMISSION | TABLE | -
PROFILE | TABLE | -
PROFILE_PERMISSION | TABLE | -
PROFILE_SCREEN | TABLE | -
SCREEN | TABLE | -
USERS | TABLE | -
USER_DEPARTMENT | TABLE | -
USER_PROFILE | TABLE | -

========================================
3. Describe Table (firebird_describe_table)
========================================
### Table: USERS

#### Columns

NAME | TYPE | NULL | DEFAULT | POSITION
--- | --- | --- | --- | ---
ID | INTEGER | NO | - | 0
DEPARTMENT_ID | INTEGER | NO | - | 1
NAME | VARCHAR(100) | NO | - | 2
EMAIL | VARCHAR(200) | NO | - | 3
PASSWORD_HASH | VARCHAR(500) | NO | - | 4
ACTIVE      BOOLEAN  | NO | TRUE | 5
SALARY | NUMERIC(14,2) | NO | 0 | 6
HIRED_AT | DATE | NO | - | 7
CREATED_AT | TIMESTAMP | NO | CURRENT_TIMESTAMP | 8
UPDATED_AT | TIMESTAMP | NO | CURRENT_TIMESTAMP | 9

========================================
4. Query (firebird_query)
========================================
### Query Results (10 rows)

ID | NAME | EMAIL | ACTIVE | SALARY
--- | --- | --- | --- | ---
1 | Alice Johnson | alice.johnson@company.com | 1 | 95000.00
2 | Bob Smith | bob.smith@company.com | 1 | 78000.50
...

========================================
5. Count (firebird_count)
========================================
Count: 10 rows in USERS

========================================
6. Code Gen — Lazarus (firebird_generate_dev)
========================================
object FormUsers: TFormManutencao
...

========================================
7. Code Gen — OpenAPI/Swagger (firebird_generate_dev)
========================================
  Users:
    type: object
    description: Schema for table USERS
    required:
      - id
      - departmentId
      - name
      - email
      - passwordHash
      - active
      - salary
      - hiredAt
      - createdAt
      - updatedAt
    properties:
      id:
        type: integer
        format: int32
      departmentId:
        type: integer
        format: int32
        x-foreign-key DEPARTMENT.ID
      name:
        type: string
      email:
        type: string
      passwordHash:
        type: string
      active:
        type: boolean
      salary:
        type: string
      hiredAt:
        type: string
        format: date
      createdAt:
        type: string
        format: date-time
      updatedAt:
        type: string
        format: date-time

========================================
8. Code Gen — Java with file-save (outputDirPath)
========================================
📄 1 file written to /tmp/mcp-out/test_mcp_java/
   - Users.java (1.8 KB, 45 lines)

Preview of Users.java:
│ import jakarta.persistence.*;
│ import lombok.*;
│
│ @Entity
│ @Table(name = "USERS")
│ @Data
│ @NoArgsConstructor
│ @AllArgsConstructor
│ @Builder
│ public class Users {
│     @Id
│     private Integer id;
│     @ManyToOne
│     private Department department;
│     private String name;
│     private String email;
│     private String passwordHash;
│     private Boolean active;  // BOOLEAN → Boolean (native Firebird 5)
│     private BigDecimal salary;
│     private LocalDate hiredAt;
│     private LocalDateTime createdAt;
│     private LocalDateTime updatedAt;
│ }
```

> **Note:** When `outputDirPath` is provided (or `storage.dir` is configured in
> `config.json`), the generated code is written to disk and the response
> contains a summary with file paths and previews instead of the full
> content. Without `outputDirPath`, the code is returned inline (legacy behavior).

## How It Works

The `uMCP` unit does **not** use the `TProcess` class (which has a limited API in FPC 3.2.2 — no `WaitFor`, `OutputLines`, or `CloseInput`). Instead, it uses raw POSIX system calls via `external 'libc'`:

```
parent process                          child process
─────────────                          ─────────────
pipe() ──────────────────────────────►  (stdin, stdout)
fork() ──────────────────────────────►  execv(MCP_BINARY)
write(pipe_in, request) ────────────►   reads JSON-RPC from stdin
read(pipe_out, response) ◄──────────    writes JSON-RPC to stdout
waitpid() ◄────────────────────────    exits
```

Each MCP call spawns a new process. The MCP server exits when stdin reaches EOF, which is what makes the pipe approach work.

## Using in a Lazarus Project

1. Copy `uMCP.pas` into your project directory
2. Add `uMCP` to the `uses` clause of your unit
3. Call `MCPCall` or `MCPListTools`:

```pascal
uses
  uMCP, SysUtils;

begin
  Writeln(MCPListTools);
  Writeln(MCPCall('firebird_count', '{"table":"USERS"}'));
end.
```

> **Note:** The `MCP_BINARY` constant is hardcoded in `uMCP.pas`. For a production app, consider making it configurable (e.g., via command-line parameters or an INI file).

## Migration Tools Example (Parquet Export/Import)

The `test_migration.pas` example demonstrates the migration tools:

1. **Export** — `firebird_export` exports the `USERS` table to `/tmp/users_test.parquet`
2. **Describe** — `firebird_describe_file` shows the Parquet schema (no database connection needed)
3. **Import** — `firebird_import` creates a new table `USERS_MIGRATION_TEST` from the Parquet file
4. **Verify** — `firebird_count` and `firebird_query` confirm the data was imported
5. **Cleanup** — `firebird_drop` removes the test table

> **Note:** The `output_path` and `input_path` must be within an allowed path (configured in `config.json` under `allowed_paths`).
