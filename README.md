# firebird-5-mcp

A lightweight, pure-Rust MCP (Model Context Protocol) server for Firebird 5 databases. No external dependencies — no `isql`, no DLLs, no `.so` files. Just a single static binary.

## Features

### Query & Data
- `firebird_query` — Execute any SQL statement (SELECT, INSERT, UPDATE, DELETE, DDL)
- `firebird_count` — Count rows in a table (with optional WHERE)
- `firebird_sample` — Quick sample of table data
- `firebird_insert_batch` — Batch insert multiple rows in one transaction
- `firebird_run_script` — Execute multi-statement SQL scripts
- `firebird_execute_immediate` — Execute statements with SET TERM blocks (procedures, functions, generators)

### Schema & Structure
- `firebird_describe_table` — Full schema of a table (columns, indexes, constraints, triggers, comments)
- `firebird_describe_database` — Complete database overview with Health section (gstat-like) + Mermaid ER diagram
- `firebird_describe` — Multi-object describe (tables, views, procedures, functions, triggers, sequences, domains)
- `firebird_list` — List tables, views, procedures, functions, domains, triggers, indexes, sequences
- `firebird_find_column` — Find columns by name pattern (LIKE) across all tables/views
- `firebird_check_duplicates` — Check for duplicate values before creating unique index/constraint

### DDL & Advanced
- `firebird_create_database` — Create a new Firebird database file
- `firebird_create_trigger` — Create BEFORE/AFTER INSERT/UPDATE/DELETE triggers
- `firebird_create_procedure` — Create stored procedures (no isql needed)
- `firebird_create_function` — Create user-defined functions
- `firebird_create_sequence` — Create sequences (generators)
- `firebird_alter_column_type` — Change column type using domains
- `firebird_drop` — Drop tables, views, domains, triggers, procedures, functions, indexes, sequences

### Code Generation
- `firebird_generate_dev` — Generate development code from table schemas:
  - **Java** (JPA + Lombok)
  - **Go** (sqlx)
  - **Rust** (rsfbclient)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (YAML schemas)
  - **GraphQL** (schema + Node.js resolvers)
  - **Lazarus/Delphi** (.lfm forms with TFBDataSet)
  - **Svelte 5** (Page + Form using `@edujed/jedsvelted-ui`)
- `firebird_generate_view_dto` — Generate DTOs/records from views (resolved metadata):
  - **Java** (Lombok @Data)
  - **Go** (sqlx tags)
  - **Rust** (FromRow)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (YAML schemas)
  - **GraphQL** (schema + Node.js resolvers, Query only)
  - **Lazarus/Delphi** (.lfm forms with TFBDataSet)
  - **Svelte 5** (Page read-only using `@edujed/jedsvelted-ui`)

### Discovery
- `firebird_get_databases` — List configured databases
- `firebird_metadata_extract` — Full DDL extraction (replaces `isql -x`)
- `firebird_explain_plan` — Get execution plan without running the query (rolled-back transaction)

### Introspection
- `firebird_table_stats` — Per-table row counts (single table or all tables)
- `firebird_users` — List all users with database access (creators + grants)
- `firebird_grants` — Show GRANTs for a table or all tables
- `firebird_blob_info` — List BLOB columns with subtype (TEXT/BINARY)
- `firebird_index_usage` — Show all indexes with columns, uniqueness, and type

> **⚠️ Not recommended for production databases.**
> This MCP is designed for development and analysis workflows. Tools like code generation,
> DDL operations, and refactoring instructions can be destructive. For production,
> use a dedicated admin tool (e.g. `isql`, `fbguard`, or your DBA's preferred suite).

## Prerequisites

- **Firebird 5** server running (or any version 3+)
- **Rust** toolchain (only if building from source)
- No `isql`, no ODBC, no native libraries required

## Setup

### 1. Configure your server and databases

Create a `config.json` file:

```json
{
  "server": {
    "host": "localhost",
    "port": 3050,
    "user": "SYSDBA",
    "password": "masterkey",
    "allowed_paths": ["/path/to/databases"],
    "databases": [
      {
        "name": "examples",
        "description": "Local development database for tests, training and examples",
        "path": "/path/to/databases/teste.fdb"
      },
      {
        "name": "myDatabase",
        "description": "My primary local development database",
        "path": "/path/to/databases/myDatabase.fdb"
      }
    ],
    "default_database": "examples"
  }
}
```

| Field | Description |
|-------|-------------|
| `host` | Firebird server hostname |
| `port` | Firebird server port (default 3050) |
| `user` | Database username |
| `password` | Database password |
| `allowed_paths` | Directories where new databases can be created |
| `databases` | List of configured databases |
| `default_database` | Database to use when none is specified |

### 2. Build

Use the build script from the project root:

```bash
./scripts/build.sh              # Current platform (binaries in bin/)
./scripts/build.sh all          # All supported platforms
```

The script also packages the binaries into `dist/` as compressed archives:

- **Windows** → `.zip` (e.g. `firebird-5-mcp-windows-amd64.zip`)
- **Linux** → `.tar.gz` (e.g. `firebird-5-mcp-linux-amd64.tar.gz`)

```bash
./scripts/build.sh all
# Binaries in: bin/
# Archives in: dist/
```

Or build manually:

```bash
cargo build --release
# Binary will be at: target/release/firebird-5-mcp
```

**Cross-compilation notes:**

| Platform | Target | Requirements |
|---|---|---|
| linux/amd64 | `x86_64-unknown-linux-gnu` | None (native on x86_64) |
| linux/arm64 | `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu-gcc` (`sudo apt install gcc-aarch64-linux-gnu`) |
| windows/amd64 | `x86_64-pc-windows-gnu` | `x86_64-w64-mingw32-gcc` (`sudo apt install gcc-mingw-w64-x86-64`) |
| darwin/* | `*-apple-darwin` | [osxcross](https://github.com/tpoechtrager/osxcross) (skipped with a warning if not installed) |
| windows/arm64 | — | Not supported (no stable GNU target in Rust) |

### 3. Register in Editors

#### 3.1. Zed Editor

Add to your Zed settings (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "enabled": true,
      "timeout": 30000
    }
  }
}
```

#### 3.2. VSCode

In vscode, open the palete command ('Ctrl+Shift+P') and execute: 'MCP: Add Server'.
Choice 'Command (stdio), and insert the binary (uncompressed) with path (example: `/path/to/firebird-5-mcp`)
Inform the server name `firebird-5-mcp`, and choice from 'personal/user' or 'project/workspace'.
confirm that you 'trust in the server' (when asked), and enable the tools in 'Configure Tools'.

If you prefer, is possible to configure from `mpc-config.json` (like `/usr/folder/.copilot/mpc-config.json`)
and type

```json
{
	"mcpServers": {
		"firebird-5-mcp": {
			"type": "local",
			"command": "/path/to/firebird-5-mcp",
			"args": [],
			"tools": ["*"]	
		}
	}
}
```

#### 3.3. Cursor

Create a `.cursor/mcp.json` file in your project root (or `~/.cursor/mcp.json` for global):

```json
{
  "mcpServers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "args": []
    }
  }
}
```

#### 3.4. JetBrains (IntelliJ, GoLand, WebStorm, CLion)

In the AI Assistant settings, add a new MCP server:

- **Command:** `/path/to/firebird-5-mcp`

Or edit the MCP config file (`~/.jetbrains/mcp.json` or project-level `.mcp.json`):

```json
{
  "mcpServers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "args": []
    }
  }
}
```

> **Note:** The binary resolves `config.json` automatically from `~/.firebird-mcp/config.json` (or `FIREBIRD_CONFIG` / `FIREBIRD_MCP_HOME` env vars). No environment variables are needed in the editor config unless you use a non-standard path.

## Using from Command Line (No IDE Required)

The MCP server speaks JSON-RPC over stdio. You can call any tool directly from the shell — useful for scripting, CI pipelines, or IDEs without MCP support (e.g. Lazarus).

### How it works

1. The JSON-RPC request is piped to the server's **stdin**
2. The server processes the tool call and writes the JSON-RPC response to **stdout**
3. When stdin reaches EOF, the server exits
4. `fpc` or `python3` extracts the `result.content[0].text` field from the response

> **Note:** The `timeout 10` prevents hanging if the server doesn't exit. The `2>/dev/null` suppresses stderr (diagnostics).

### Helper scripts

Ready-to-use scripts are in [`examples/`](examples/):

| Script | Platform | Usage |
|--------|----------|-------|
| [`mcp-call.sh`](examples/mcp-call.sh) | Linux/macOS | `./mcp-call.sh <tool> '<json_args>'` |
| [`mcp-call.bat`](examples/mcp-call.bat) | Windows | `mcp-call.bat <tool> <json_args>` |

**Examples (Linux/macOS):**

```bash
# List tables
./examples/mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Query data
./examples/mcp-call.sh firebird_query '{"sql":"SELECT * FROM USERS","database":"test-mcp"}'

# Describe a table
./examples/mcp-call.sh firebird_describe_table '{"table_name":"USERS","database":"test-mcp"}'

# Generate code (OpenAPI)
./examples/mcp-call.sh firebird_generate_dev '{"tables":["USERS"],"language":"openapi","database":"test-mcp"}'
```

Set `MCP_BINARY` in the script (or as an environment variable) to point to your binary. The config file is resolved automatically by the binary (`~/.firebird-mcp/config.json` by default).

### Lazarus / Free Pascal Integration

For IDEs without MCP support (like Lazarus), you can call the MCP server directly from Pascal code. The example below uses **raw POSIX pipes** (`fork`/`exec`/`read`/`write`) instead of `TProcess`, because the `TProcess` API in FPC 3.2.2 is limited (no `WaitFor`, `OutputLines`, or `CloseInput`).

A complete, tested example is in [`examples/freePascal/`](examples/freePascal/).

**Build & run the test program:**

```bash
cd examples/freePascal
fpc -otest_mcp test_mcp.pas
./test_mcp
```

> **Note:** Each `MCPCall` spawns a new process. For high-frequency calls, consider keeping a persistent connection (write to stdin, read from stdout in a thread) — but for typical dev workflows, the spawn-per-call approach is simple and sufficient.

## Configuration

### `config.json`

The configuration file defines the Firebird server connection and the list of databases available to the MCP tools.

```json
{
  "server": {
    "host": "localhost",
    "port": 3050,
    "user": "SYSDBA",
    "password": "masterkey",
    "allowed_paths": ["/path/to/databases"],
    "databases": [
      {
        "name": "examples",
        "description": "Local development database",
        "path": "/path/to/databases/examples.fdb"
      }
    ],
    "default_database": "examples"
  }
}
```

| Field | Description |
|-------|-------------|
| `host` | Firebird server hostname |
| `port` | Firebird server port (default 3050) |
| `user` | Database username |
| `password` | Database password |
| `allowed_paths` | Directories where new databases can be created |
| `databases` | List of configured databases |
| `default_database` | Database to use when none is specified |

### Database entry

Each entry in the `databases` array:

```json
{
  "name": "examples",
  "description": "Local development database",
  "path": "/path/to/databases/examples.fdb"
}
```

### Configuration file lookup

The server looks for `config.json` in this order:
1. `FIREBIRD_CONFIG` environment variable (full path)
2. `./config.json` (current working directory)
3. `~/.firebird-mcp/config.json` (default)

The base directory can be overridden with `FIREBIRD_MCP_HOME` (e.g. for testing or non-standard installs).

### Environment variables

| Variable | Description |
|----------|-------------|
| `FIREBIRD_CONFIG` | Full path to `config.json` (highest priority) |
| `FIREBIRD_MCP_HOME` | Override the base directory (default: `~/.firebird-mcp`) |

## Tools

### `firebird_query`

Executes a single SQL statement. For DML/DDL, the execution plan is appended to the result.

**Arguments:**
- `sql` (required): The SQL statement
- `database` (optional): Database name (defaults to `default_database`)

**Example:**
```json
{
  "sql": "SELECT * FROM CLIENTS WHERE CITY = 'New York'",
  "database": "shop"
}
```

**Result:**
```
### Query Results (3 rows)

Columns: ID (INT), NAME (VARCHAR), CITY (VARCHAR)

ID | NAME | CITY
--- | --- | ---
1 | Alice | New York
2 | Bob | NULL
3 | Charlie | New York
```

### `firebird_list`

Lists database objects by type.

**Arguments:**
- `type` (required): TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, or SEQUENCES
- `database` (optional): Database name

**Example:**
```json
{
  "type": "TABLES",
  "database": "shop"
}
```

**Result:**
```
### Tables and Views (5)

NAME | TYPE | COMMENT
--- | --- | ---
CLIENTS | TABLE | Customer records
ORDERS | TABLE | Order data
...
```

### `firebird_find_column`

Finds columns by name pattern across all tables and views. Supports `%` wildcards.

**Arguments:**
- `column` (required): Column name pattern (supports `%` wildcards). E.g., `EMAIL`, `%NOME%`, `ID`
- `database` (optional): Database name

**Example:**
```json
{
  "column": "%EMAIL%",
  "database": "shop"
}
```

**Result:**
```
### Columns matching '%EMAIL%' (2 found)

TABLE | COLUMN | TYPE | NULL | VIEW
--- | --- | --- | --- | ---
USERS | EMAIL | VARCHAR(250) | NO |
VW_ACTIVE_USERS | EMAIL | VARCHAR(250) | YES | YES
```

### `firebird_check_duplicates`

Checks for duplicate values in one or more columns. Use this BEFORE creating a unique index or constraint to verify data integrity. Supports composite keys.

**Arguments:**
- `table` (required): Table name to check
- `columns` (required): Array of column names to check (supports composite keys)
- `limit` (optional): Maximum number of duplicate combinations to return (default: 10)
- `database` (optional): Database name

**Example:**
```json
{
  "table": "CUSTOMERS",
  "columns": ["TYPE_ID"],
  "database": "shop"
}
```

**Result (with duplicates):**
```
### Duplicate Check: CUSTOMERS (TYPE_ID)

⚠️ 1 duplicate combination(s) found:

TYPE_ID | DUPLICATED_COUNT
--- | ---
2 | 5

#### Full rows for first duplicate (TYPE_ID = 2):

ID | TYPE_ID | PARENT_ID | IS_VERIFIED | ACTIVE | ...
--- | --- | --- | --- | --- | ---
2 | 2 | NULL | false | true | ...
3 | 2 | 2 | true | true | ...
4 | 2 | NULL | false | true | ...
5 | 2 | 4 | true | true | ...
6 | 2 | 4 | false | true | ...

💡 Fix duplicates before creating the unique index/constraint.
```

**Result (no duplicates):**
```
### Duplicate Check: CUSTOMERS (ID)

✅ No duplicates found. Safe to create unique index/constraint.
```

### `firebird_describe_table`

Shows the full schema of a table or view.

**Arguments:**
- `table_name` (required): Table or view name
- `database` (optional): Database name

**Result:**
```
### Table: CLIENTS

#### Columns

NAME | TYPE | NULL | DEFAULT | POSITION
--- | --- | --- | --- | ---
ID | INTEGER | NO | - | 1
NAME | VARCHAR(100) | NO | - | 2
EMAIL | VARCHAR(255) | YES | - | 3
CREATED_AT | TIMESTAMP | NO | CURRENT_TIMESTAMP | 4

#### Indexes

NAME | UNIQUE | TYPE | COLUMNS
--- | --- | --- | ---
PK_CLIENTS | YES | 0 | ID

#### Constraints

NAME | TYPE
--- | ---
PK_CLIENTS | PRIMARY KEY
```

### `firebird_describe_database`

Provides a complete overview of the database structure with a Health section (gstat-like info) and a Mermaid ER diagram.

**Arguments:**
- `database` (optional): Database name
- `include_counts` (optional): Include row counts (may be slow)
- `include_describes` (optional): Include column details

**Result:**
```
### Database: shop

#### Health

- **Path:** /database/shop.fdb
- **ODS Version:** 13.1
- **Page Size:** 32768 bytes
- **File Size:** 428 pages (~13.4 MB)
- **Dialect:** 3
- **Mode:** Online (Normal)
- **Read-Only:** NO
- **Forced Writes:** YES
- **Active Connections:** 1
- **Oldest Transaction:** 1063 (active: 1772, next: 1772)
- **Buffer Pool:** 2048 pages

#### Tables (26)

**USERS**
- ID (INTEGER, PK)
...

#### Relationships (Mermaid)

```mermaid
erDiagram
    CUSTOMERS ||--o{ USERS : "FK_USR_CUST"
    ...
```
```

**Health section fields:**
| Field | Description |
|-------|-------------|
| Path | Database file path |
| ODS Version | On-Disk Structure version (13.1 = Firebird 5) |
| Page Size | Page size in bytes |
| File Size | Total pages and estimated size in MB |
| Dialect | SQL dialect (3 = ANSI) |
| Mode | Shutdown mode: Online (Normal), Multi-user, Single-user, Full |
| Read-Only | Whether the database is read-only |
| Forced Writes | Whether forced writes are enabled |
| Active Connections | Number of active user connections |
| Oldest Transaction | Oldest transaction ID (active and next) |
| Buffer Pool | Number of buffer pool pages |

### `firebird_describe`

Multi-object describe — accepts any mix of tables, views, procedures, functions, triggers, sequences, and domains.

**Arguments:**
- `objects` (required): Array of object names
- `database` (optional): Database name

**Example:**
```json
{
  "objects": ["SAVE_USER", "USERS", "D_BOOL"],
  "database": "shop"
}
```

**Result (procedure with parameters):**
```
### SAVE_USER (PROCEDURE)

#### Input Parameters

POSITION | NAME | DOMAIN
--- | --- | ---
0 | P_OPER | RDB$33
1 | P_ID | D_ID_NULL
2 | P_EMAIL | D_VC255
...

#### Output Parameters

POSITION | NAME | DOMAIN
--- | --- | ---
0 | ID | D_ID
1 | OK | D_BOOL

#### Source

```
CREATE PROCEDURE SAVE_USER (P_OPER RDB$33, P_ID D_ID_NULL, ...)
AS
DECLARE VARIABLE V_AFF INT;
BEGIN
    ...
END
```
```

### `firebird_generate_dev`

Generates development code from table schemas.

**Arguments:**
- `tables` (required): Array of table names (max 20)
- `language` (required): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `graphql`, `lazarus`, or `svelte`
- `database` (optional): Database name
- `include_fks` (optional): If true, generates named FK relations (Java `@ManyToOne`) or FK comments

**Example (Go):**
```json
{
  "tables": ["USERS"],
  "language": "go",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```go
// User maps to table USERS
type User struct {
    Id           sql.NullInt32  `db:"ID" json:"id"`
    CustomerId   sql.NullInt32  `db:"CUSTOMER_ID" json:"customerId"` // FK → CUSTOMERS.ID
    Email        sql.NullString `db:"EMAIL" json:"email"`
    PasswordHash sql.NullString `db:"PASSWORD_HASH" json:"passwordHash"`
    Active       bool           `db:"ACTIVE" json:"active"`
    CreatedAt    *time.Time     `db:"CREATED_AT" json:"createdAt"`
    UpdatedAt    *time.Time     `db:"UPDATED_AT" json:"updatedAt"`
    CreatedBy    i32            `db:"CREATED_BY" json:"createdBy"`
    UpdatedBy    i32            `db:"UPDATED_BY" json:"updatedBy"`
    DeletedAt    *time.Time     `db:"DELETED_AT" json:"deletedAt"`
}
```

**Example (Java):**
```json
{
  "tables": ["USERS"],
  "language": "java",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```java
@Entity
@Table(name = "USERS")
@Data
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class User {

    @Id
    @Column(name = "ID")
    private Integer id;

    @ManyToOne
    @JoinColumn(name = "CUSTOMER_ID")
    private Customer customer;

    @Column(name = "EMAIL", length = 250)
    private String email;

    @Column(name = "PASSWORD_HASH", length = 250)
    private String passwordHash;

    @Column(name = "ACTIVE")
    private Boolean active;

    @Column(name = "CREATED_AT")
    private LocalDateTime createdAt;

    @Column(name = "UPDATED_AT")
    private LocalDateTime updatedAt;

    @Column(name = "CREATED_BY")
    private Integer createdBy;

    @Column(name = "UPDATED_BY")
    private Integer updatedBy;

    @Column(name = "DELETED_AT")
    private LocalDateTime deletedAt;
}
```

**Example (Rust):**
```json
{
  "tables": ["USERS"],
  "language": "rust",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```rust
// User maps to table USERS
#[derive(Debug, Clone, FromRow)]
pub struct User {
    /// Column: ID
    pub id: Option<i32>,
    /// Column: CUSTOMER_ID — FK → CUSTOMERS.ID
    pub customer_id: Option<i32>,
    /// Column: EMAIL
    pub email: Option<String>,
    /// Column: PASSWORD_HASH
    pub password_hash: Option<String>,
    /// Column: ACTIVE
    pub active: bool,
    /// Column: CREATED_AT
    pub created_at: NaiveDateTime,
    /// Column: UPDATED_AT
    pub updated_at: NaiveDateTime,
    /// Column: CREATED_BY
    pub created_by: i32,
    /// Column: UPDATED_BY
    pub updated_by: i32,
    /// Column: DELETED_AT
    pub deleted_at: NaiveDateTime,
}

// Example query (aliases must match field names for FromRow):
// SELECT ID AS id, CUSTOMER_ID AS customer_id, EMAIL AS email, ... FROM USERS
// let rows: Vec<User> = conn.query(sql, ({}))?;
```

**Example (TypeScript):**
```json
{
  "tables": ["USERS"],
  "language": "typescript",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```typescript
export interface User {
    id?: number;
    /** FK → CUSTOMERS.ID */
    customerId?: number;
    email?: string;
    passwordHash?: string;
    active: boolean;
    createdAt: string;
    updatedAt: string;
    createdBy: number;
    updatedBy: number;
    deletedAt: string;
}
```

**Example (Protobuf):**
```json
{
  "tables": ["USERS"],
  "language": "protobuf",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```protobuf
syntax = "proto3";

package users;

// Nullable columns: ID, CUSTOMER_ID, EMAIL, PASSWORD_HASH
message User {
    int32 id = 1;
    // FK → CUSTOMERS.ID
    int32 customer_id = 2;
    string email = 3;
    string password_hash = 4;
    bool active = 5;
    string created_at = 6;
    string updated_at = 7;
    int32 created_by = 8;
    int32 updated_by = 9;
    string deleted_at = 10;
}
```

**Example (OpenAPI):**
```json
{
  "tables": ["USERS"],
  "language": "openapi",
  "database": "shop",
  "include_fks": true
}
```

**Result:**
```yaml
User:
  type: object
  description: Schema for table USERS
  required:
    - active
    - createdAt
    - updatedAt
    - createdBy
    - updatedBy
    - deletedAt
  properties:
    id:
      type: integer
      format: int32
      nullable: true
    customerId:
      type: integer
      format: int32
      nullable: true
      x-foreign-key: CUSTOMERS.ID
    email:
      type: string
      nullable: true
    passwordHash:
      type: string
      nullable: true
    active:
      type: boolean
    createdAt:
      type: string
      format: date-time
    updatedAt:
      type: string
      format: date-time
    createdBy:
      type: integer
      format: int32
    updatedBy:
      type: integer
      format: int32
    deletedAt:
      type: string
      format: date-time
```

**Example (Lazarus/Delphi):**
```json
{
  "tables": ["CUSTOMERS"],
  "language": "lazarus",
  "database": "shop"
}
```

**Result:**
```lfm
object FormCustomer: TFormManutencao
  Left = 0
  Top = 0
  Caption = 'FormCustomer'
  ClientHeight = 400
  ClientWidth = 600
  LCLVersion = '3.0.0.0'
  object FBDataSetCustomer: TFBDataSet
    Left = 24
    Top = 24
    Connected = False
    SQL.Strings = (
      'SELECT * FROM CUSTOMERS'
    )
  end
  object DataSourceCustomer: TDataSource
    Left = 112
    Top = 24
    DataSet = FBDataSetCustomer
  end
  object Label_ID: TLabel
    Left = 24
    Top = 100
    Width = 120
    Height = 15
    Caption = 'ID:'
    ParentColor = False
  end
  object TDBEdit_ID: TDBEdit
    Left = 24
    Top = 121
    Width = 120
    Height = 23
    DataSource = DataSourceCustomer
    DataField = 'ID'
    TabOrder = 0
  end
  object Label_IS_VERIFIED: TLabel
    Left = 24
    Top = 160
    Width = 150
    Height = 15
    Caption = 'IS_VERIFIED:'
    ParentColor = False
  end
  object TDBCheckBox_IS_VERIFIED: TDBCheckBox
    Left = 24
    Top = 181
    Width = 150
    Height = 23
    DataSource = DataSourceCustomer
    DataField = 'IS_VERIFIED'
    TabOrder = 1
  end
  object Label_CREATED_AT: TLabel
    Left = 24
    Top = 220
    Width = 150
    Height = 15
    Caption = 'CREATED_AT:'
    ParentColor = False
  end
  object TDBDateTimePicker_CREATED_AT: TDBDateTimePicker
    Left = 24
    Top = 241
    Width = 150
    Height = 23
    DataSource = DataSourceCustomer
    DataField = 'CREATED_AT'
    TabOrder = 2
    Kind = dtkDate
  end
  ...
end
```

**Component mapping:**
| Firebird Type | Lazarus Component | Width |
|---------------|-------------------|-------|
| INTEGER/BIGINT/SMALLINT | TDBEdit | 120 |
| VARCHAR/CHAR | TDBEdit | 80/250/450 (dynamic) |
| NUMERIC/DECIMAL | TDBEdit | 120 |
| BOOLEAN | TDBCheckBox | 150 |
| DATE/TIMESTAMP | TDBDateTimePicker | 150 |
| BLOB SUB_TYPE TEXT | TDBMemo | 450 |
| BLOB SUB_TYPE BINARY | TDBEdit | 250 |

**Example (Svelte 5):**
```json
{
  "tables": ["CUSTOMERS"],
  "language": "svelte",
  "database": "shop"
}
```

**Result:** Generates two components:

1. **Page** — `PageShell` + `Table` with search by PK and FKs:
```svelte
<script lang="ts">
	import { PageShell } from '@edujed/jedsvelted-ui/pages';
	import { Table } from '@edujed/jedsvelted-ui/table';
	import type { TableCol, TableAction } from '@edujed/jedsvelted-ui/table';
	import { localeStore } from '@edujed/jedsvelted-ui/i18n';
	import { formatDate } from '@edujed/jedsvelted-ui/format';

	let data = $state<any[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);

	// Search filters (PK + FKs)
	let searchID = $state('');
	let searchPARENT_ID = $state('');
	let searchTYPE_ID = $state('');

	const columns: TableCol[] = [
		{ key: 'id', title: 'Id', sortable: true },
		{ key: 'typeId', title: 'TypeId' },
		{ key: 'parentId', title: 'ParentId' },
		{ key: 'isVerified', title: 'IsVerified' },
		{ key: 'active', title: 'Active' },
		{ key: 'createdAt', title: 'CreatedAt', formatter: (v: string) => formatDate(v, $localeStore) },
		// ...
	];

	const actions: TableAction[] = [
		{ title: 'View', icon: 'eye', onClick: (row) => showDetail(row) },
		{ title: 'Edit', icon: 'edit', onClick: (row) => editRow(row) },
		{ title: 'Delete', icon: 'trash', onClick: (row) => deleteRow(row) },
	];

	async function search() { /* TODO: API call */ }
	function clear() { /* resets search fields */ }
	function showDetail(row: any) { /* TODO */ }
	function editRow(row: any) { /* TODO */ }
	function deleteRow(row: any) { /* TODO */ }
</script>

<PageShell
	title="Customer"
	onSearch={search}
	onClear={clear}
	{loading}
	{error}
	skeletonVariant="table"
	skeletonRows={5}
>
	{#snippet searchFields()}
		<div class="search-fields">
			<label>
				Id
				<input type="text" bind:value={searchID} />
			</label>
			<!-- FK search fields -->
		</div>
	{/snippet}

	{#snippet content(state)}
		<Table
			id="customer"
			caption="Customer"
			{data}
			{columns}
			{actions}
			rowKey="id"
			csvFileName="customer.csv"
			onAdd={() => {}}
		/>
	{/snippet}
</PageShell>
```

2. **Form** — Fields with `$state` (no event logic):
```svelte
<script lang="ts">
	import {
		EditField,
		NumericField,
		DateField,
		SwitchField,
		FormActions
	} from '@edujed/jedsvelted-ui/forms';

	let id = $state<number>(0);
	let typeId = $state<number>(0);
	let parentId = $state<number>(0);
	let isVerified = $state<boolean>(false);
	let active = $state<boolean>(false);
	let createdAt = $state<string>('');
	// ...

	function save() { /* TODO */ }
	function cancel() { /* TODO */ }
</script>

<form>
	<NumericField label="Id" bind:value={id} />
	<NumericField label="TypeId" bind:value={typeId} />
	<NumericField label="ParentId" bind:value={parentId} />
	<SwitchField label="IsVerified" bind:value={isVerified} />
	<SwitchField label="Active" bind:value={active} />
	<DateField label="CreatedAt" bind:value={createdAt} />
	<!-- ... -->
	<FormActions onSave={save} onCancel={cancel} />
</form>
```

**Field mapping:**
| Firebird Type | Svelte Component | TS Type |
|---------------|-----------------|--------|
| BOOLEAN | SwitchField | boolean |
| DATE/TIMESTAMP | DateField | string |
| INTEGER/BIGINT/SMALLINT/NUMERIC/DECIMAL | NumericField | number |
| VARCHAR/CHAR/BLOB | EditField | string |

### `firebird_generate_view_dto`

Generates DTOs/records from views using resolved metadata (executes `SELECT FIRST 1 * FROM view` to get accurate column types). Generated code is read-only (no entity annotations, no FKs).

**Arguments:**
- `views` (required): Array of view names (max 20)
- `language` (required): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `graphql`, `lazarus`, or `svelte`
- `database` (optional): Database name

**Example (Java):**
```json
{
  "views": ["VW_ACTIVE_USERS"],
  "language": "java",
  "database": "shop"
}
```

**Result:**
```java
@Data
public class VwActiveUsers {
    private Long id;
    private String email;
    private String passwordHash;
    private Boolean active;
    private Long customerId;
    private Long typeId;
    private Boolean isActive;
    private String fullName;
    private java.time.LocalDateTime createdAt;
    private java.time.LocalDateTime updatedAt;
    private String createdBy;
    private String updatedBy;
}
```

**Example (Lazarus/Delphi):**
```json
{
  "views": ["VW_ACTIVE_USERS"],
  "language": "lazarus",
  "database": "shop"
}
```

**Result:**
```lfm
object FormVwActiveUsers: TFormConsulta
  Left = 0
  Top = 0
  Caption = 'FormVwActiveUsers'
  ClientHeight = 400
  ClientWidth = 600
  LCLVersion = '3.0.0.0'
  object FBDataSetVwActiveUsers: TFBDataSet
    Left = 24
    Top = 24
    Connected = False
    SQL.Strings = (
      'SELECT * FROM VW_ACTIVE_USERS'
    )
  end
  object DataSourceVwActiveUsers: TDataSource
    Left = 112
    Top = 24
    DataSet = FBDataSetVwActiveUsers
  end
  object Label_ID: TLabel
    Left = 24
    Top = 100
    Width = 120
    Height = 15
    Caption = 'ID:'
    ParentColor = False
  end
  object TDBEdit_ID: TDBEdit
    Left = 24
    Top = 121
    Width = 120
    Height = 23
    DataSource = DataSourceVwActiveUsers
    DataField = 'ID'
    TabOrder = 0
  end
  object Label_ACTIVE: TLabel
    Left = 24
    Top = 160
    Width = 150
    Height = 15
    Caption = 'ACTIVE:'
    ParentColor = False
  end
  object TDBCheckBox_ACTIVE: TDBCheckBox
    Left = 24
    Top = 181
    Width = 150
    Height = 23
    DataSource = DataSourceVwActiveUsers
    DataField = 'ACTIVE'
    TabOrder = 1
  end
  ...
end
```

### `firebird_explain_plan`

Gets the execution plan for a SQL statement without actually running it. The statement is executed inside a rolled-back transaction.

**Arguments:**
- `sql` (required): The SQL statement
- `database` (optional): Database name

**Result:**
```
Execution plan (the statement was executed inside a rolled-back transaction):
PLAN:
  SORT
    TABLE: CLIENTS (INDEX: PK_CLIENTS)
```

### `firebird_metadata_extract`

Extracts complete database metadata (DDL) using system tables. Replaces `isql -x`.

**Arguments:**
- `database` (optional): Database name
- `objects` (optional): Array of object names to filter (e.g., `["CLIENTS", "ORDERS"]`)

**Result:**
```sql
CREATE TABLE CLIENTS (
    ID INTEGER NOT NULL,
    NAME VARCHAR(100) NOT NULL,
    EMAIL VARCHAR(255),
    CREATED_AT TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT PK_CLIENTS PRIMARY KEY (ID)
);

CREATE INDEX IDX_CLIENTS_EMAIL ON CLIENTS (EMAIL);
...
```

### `firebird_table_stats`

Returns per-table row counts. For a single table, uses `SELECT COUNT(*) FROM <table>`. For all tables, uses an `EXECUTE BLOCK` with `EXECUTE STATEMENT` to dynamically count each table.

**Arguments:**
- `table` (optional): Table name. Omit to get stats for all tables.
- `database` (optional): Database name

**Result:**
```
## Table Stats (all tables)

TABLE_NAME | TOTAL
--- | ---
CLIENTS | 1542
ORDERS | 8931
PRODUCTS | 234
```

> **Note:** `COUNT(*)` on very large tables can be slow. Intended for development/analysis.

### `firebird_users`

Lists all users with access to the database, combining `RDB$DB_CREATORS` (database owners) with `RDB$USER_PRIVILEGES` (users with explicit grants).

**Arguments:**
- `database` (optional): Database name

**Result:**
```
## Database Users

USER_NAME | CREATOR_TYPE | PRIVILEGE | GRANT_OPTION
--- | --- | --- | ---
SYSDBA | 0 |  | 
APP_USER |  | SELECT | 
APP_USER |  | INSERT | 
APP_USER |  | UPDATE | 
```

### `firebird_grants`

Shows GRANT privileges for a specific table or for all tables, using `RDB$USER_PRIVILEGES`.

**Arguments:**
- `table` (optional): Table name. Omit to show grants for all tables.
- `database` (optional): Database name

**Result:**
```
## Grants for CLIENTS

RELATION_NAME | GRANTEE | PRIVILEGE | GRANT_OPTION
--- | --- | --- | ---
CLIENTS | APP_USER | SELECT | 
CLIENTS | APP_USER | INSERT | 
CLIENTS | APP_USER | UPDATE | 
```

### `firebird_blob_info`

Lists all BLOB columns in the database (or a specific table): table, column, blob subtype (TEXT/BINARY), and field length.

**Arguments:**
- `table` (optional): Table name. Omit to list BLOB columns in all tables.
- `database` (optional): Database name

**Result:**
```
## BLOB Columns (all tables)

RELATION_NAME | FIELD_NAME | FIELD_TYPE | SUB_TYPE | LENGTH | BLOB_TYPE
--- | --- | --- | --- | --- | ---
DOCUMENTS | CONTENT | 261 | 1 | 0 | TEXT
INVOICES | PDF_DATA | 261 | 2 | 0 | BINARY
```

### `firebird_index_usage`

Shows all indexes in the database (or a specific table): index name, table, uniqueness, type, and columns.

**Arguments:**
- `table` (optional): Table name. Omit to show indexes for all tables.
- `database` (optional): Database name

**Result:**
```
## Indexes (all tables)

INDEX_NAME | RELATION_NAME | UNIQUE_FLAG | INDEX_TYPE | COLUMNS
--- | --- | --- | --- | ---
PK_CLIENTS | CLIENTS | 2 | 0 | ID
IDX_CLIENTS_EMAIL | CLIENTS | 1 | 0 | EMAIL
IDX_ORDERS_CLIENT | ORDERS | 0 | 0 | CLIENT_ID, CREATED_AT
```

| UNIQUE_FLAG | Meaning |
|---|---|
| 0 | Non-unique |
| 1 | Unique |
| 2 | Primary key |

## Project Structure

```
firebird-5-mcp/             # Repository root
├── Cargo.toml              # Workspace manifest
├── crates/
│   ├── mcp-core/           # Reusable library (firebird-5-mcp-core)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs      # Public API
│   │       ├── config.rs   # Config loading
│   │       ├── client.rs   # Connection wrapper
│   │       ├── query.rs    # SQL execution
│   │       ├── schema.rs   # Schema introspection
│   │       ├── ddl.rs      # DDL operations
│   │       ├── explain.rs  # Execution plans
│   │       ├── metadata.rs # DDL extraction
│   │       ├── generator.rs# Code generation
│   │       └── types.rs    # Type formatting
│   └── mcp-bin/            # MCP server binary (firebird-5-mcp)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs     # JSON-RPC stdio loop
│           └── tools/
│               ├── mod.rs          # Dispatch
│               ├── common.rs       # Shared helpers
│               ├── definitions.rs  # Tool schemas
│               ├── query_tools.rs  # Query tools
│               ├── schema_tools.rs # Schema tools
│               ├── ddl_tools.rs    # DDL tools
│               └── generator_tools.rs # Code generation
├── scripts/
│   ├── build.sh            # Multi-platform build
│   └── publish_new_tag.sh  # Tag + release helper
├── tests/                  # Shell integration tests
├── .github/workflows/
│   └── release.yml         # CI/CD release pipeline
├── README.md
├── CHANGELOG.md
└── LICENSE
```

### Design

The project is split into two crates:

1. **`firebird-5-mcp-core`** — A reusable library that provides:
   - Database connection management
   - SQL query execution
   - Schema introspection
   - DDL operations
   - Code generation

   This library can be used in other Rust projects (e.g., desktop apps, CLI tools) without the MCP layer.

2. **`firebird-5-mcp`** — The MCP server binary that:
   - Implements the MCP protocol (JSON-RPC over stdio)
   - Exposes the core library's functionality as MCP tools
   - Handles configuration and error reporting

### Extending with new database engines

The core library is designed to be database-agnostic. To add support for a new database engine:

1. Create a new crate (e.g., `postgres-mcp-core`)
2. Implement the same traits/interfaces
3. Create a new MCP server binary (e.g., `postgres-mcp`)

## Optimized Build for Deployment

The release profile is optimized for minimal binary size and maximum performance:

```toml
[profile.release]
opt-level = 3            # Maximum optimization
codegen-units = 1        # Best codegen (slower build, better runtime)
lto = "fat"              # Aggressive link-time optimization
panic = "abort"           # Smaller binary (no unwinding)
strip = true              # Remove symbols from binary
debug = false             # No debug info
```

**Result:** ~1.2MB static binary

## Credits

- [rsfbclient](https://crates.io/crates/rsfbclient) — Pure-Rust Firebird client driver (no native libraries required)
- [jedSvelted-ui](https://github.com/edujed/jedSvelted-ui) — Svelte 5 component library used by the Svelte code generator (`@edujed/jedsvelted-ui`)
- [Model Context Protocol](https://modelcontextprotocol.io) — The MCP specification

## License

MIT — see [LICENSE](LICENSE).
