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
- `firebird_describe_database` — Complete database overview with Mermaid ER diagram
- `firebird_describe` — Multi-object describe (tables, views, procedures, functions, triggers, sequences, domains)
- `firebird_list` — List tables, views, procedures, functions, domains, triggers, indexes, sequences

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
  - **Lazarus/Delphi** (.lfm forms with TFBDataSet)
  - **Svelte 5** (Page + Form using `@edujed/jedsvelted-ui`)
- `firebird_generate_view_dto` — Generate DTOs/records from views (resolved metadata):
  - **Java** (Lombok @Data)
  - **Go** (sqlx tags)
  - **Rust** (FromRow)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (YAML schemas)
  - **Lazarus/Delphi** (.lfm forms with TFBDataSet)
  - **Svelte 5** (Page + Form using `@edujed/jedsvelted-ui`)

### Discovery
- `firebird_get_databases` — List configured databases
- `firebird_metadata_extract` — Full DDL extraction (replaces `isql -x`)
- `firebird_explain_plan` — Get execution plan without running the query (rolled-back transaction)

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
        "name": "teste",
        "description": "Local development database",
        "path": "/path/to/databases/teste.fdb"
      },
      {
        "name": "financeiro",
        "description": "Financial database",
        "path": "/path/to/databases/financeiro.fdb"
      }
    ],
    "default_database": "teste"
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

```bash
# From the firebird-5-mcp directory
cd firebird-5-mcp
cargo build --release

# Binary will be at:
# firebird-5-mcp/target/release/firebird-5-mcp
```

Or use the build script from the project root:

```bash
./scripts/build.sh rust
# Binary will be at: bin/firebird-5-mcp-linux-amd64
```

### 3. Register in Zed Editor

Add to your Zed settings (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "enabled": true,
      "env": {
        "FIREBIRD_CONFIG": "/path/to/config.json"
      },
      "timeout": 30000
    }
  }
}
```

## Configuration

### `config.json`

Same format as the Go version. The Rust version is fully compatible with existing Go configs.

### Database entry

```json
{
  "name": "teste",
  "description": "Local development database",
  "path": "/path/to/databases/teste.fdb"
}
```

### Configuration file lookup

The server looks for `config.json` in this order:
1. `FIREBIRD_CONFIG` environment variable
2. `./config.json` (current directory)
3. `~/.config/firebird-5-mcp/config.json`

### Environment variables

| Variable | Description |
|----------|-------------|
| `FIREBIRD_CONFIG` | Path to config.json |

## Tools

### `firebird_query`

Executes a single SQL statement. For DML/DDL, the execution plan is appended to the result.

**Arguments:**
- `sql` (required): The SQL statement
- `database` (optional): Database name (defaults to `default_database`)

**Example:**
```json
{
  "sql": "SELECT * FROM CLIENTS WHERE CITY = 'São Paulo'",
  "database": "teste"
}
```

**Result:**
```
### Query Results (3 rows)

Columns: ID (INT), NAME (VARCHAR), CITY (VARCHAR)

ID | NAME | CITY
--- | --- | ---
1 | Alice | São Paulo
2 | Bob | NULL
3 | Charlie | São Paulo
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
  "database": "teste"
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

Provides a complete overview of the database structure with a Mermaid ER diagram.

**Arguments:**
- `database` (optional): Database name
- `include_counts` (optional): Include row counts (may be slow)
- `include_describes` (optional): Include column details

**Result:**
```
### Database: teste

#### Tables (19)

**DEPARTAMENTO** (12 rows)
- ID (INTEGER, PK)
- NOME (VARCHAR(400), UNIQUE)
...

#### Relationships (Mermaid)

```mermaid
erDiagram
    DEPARTAMENTO ||--o{ USUARIO : "has"
    USUARIO ||--o{ USUARIO_PERFIL : "has"
    ...
```
```

### `firebird_describe`

Multi-object describe — accepts any mix of tables, views, procedures, functions, triggers, sequences, and domains.

**Arguments:**
- `objects` (required): Array of object names
- `database` (optional): Database name

**Example:**
```json
{
  "objects": ["SAVE_USER", "AUT_USUARIO", "D_BOOL"],
  "database": "financeiro"
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
- `language` (required): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `lazarus`, or `svelte`
- `database` (optional): Database name
- `include_fks` (optional): If true, generates named FK relations (Java `@ManyToOne`) or FK comments

**Example (Go):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "go",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```go
// AutUsuario maps to table AUT_USUARIO
type AutUsuario struct {
    Id          sql.NullInt32  `db:"ID" json:"id"`
    PessoaId    sql.NullInt32  `db:"PESSOA_ID" json:"pessoaId"` // FK → PEP_PESSOA.ID
    Email       sql.NullString `db:"EMAIL" json:"email"`
    SenhaHash   sql.NullString `db:"SENHA_HASH" json:"senhaHash"`
    Ativo       bool           `db:"ATIVO" json:"ativo"`
    CreatedAt   *time.Time     `db:"CREATED_AT" json:"createdAt"`
    UpdatedAt   *time.Time     `db:"UPDATED_AT" json:"updatedAt"`
    CreatedBy   i32            `db:"CREATED_BY" json:"createdBy"`
    UpdatedBy   i32            `db:"UPDATED_BY" json:"updatedBy"`
    DeletedAt   *time.Time     `db:"DELETED_AT" json:"deletedAt"`
}
```

**Example (Java):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "java",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```java
@Entity
@Table(name = "AUT_USUARIO")
@Data
@NoArgsConstructor
@AllArgsConstructor
@Builder
public class AutUsuario {

    @Id
    @Column(name = "ID")
    private Integer id;

    @ManyToOne
    @JoinColumn(name = "PESSOA_ID")
    private PepPessoa pessoaId;

    @Column(name = "EMAIL", length = 250)
    private String email;

    @Column(name = "SENHA_HASH", length = 250)
    private String senhaHash;

    @Column(name = "ATIVO")
    private Boolean ativo;

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
  "tables": ["AUT_USUARIO"],
  "language": "rust",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```rust
// AutUsuario maps to table AUT_USUARIO
#[derive(Debug, Clone, FromRow)]
pub struct AutUsuario {
    /// Column: ID
    pub id: Option<i32>,
    /// Column: PESSOA_ID — FK → PEP_PESSOA.ID
    pub pessoa_id: Option<i32>,
    /// Column: EMAIL
    pub email: Option<String>,
    /// Column: SENHA_HASH
    pub senha_hash: Option<String>,
    /// Column: ATIVO
    pub ativo: bool,
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
// SELECT ID AS id, PESSOA_ID AS pessoa_id, EMAIL AS email, ... FROM AUT_USUARIO
// let rows: Vec<AutUsuario> = conn.query(sql, ({}))?;
```

**Example (TypeScript):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "typescript",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```typescript
export interface AutUsuario {
    id?: number;
    /** FK → PEP_PESSOA.ID */
    pessoaId?: number;
    email?: string;
    senhaHash?: string;
    ativo: boolean;
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
  "tables": ["AUT_USUARIO"],
  "language": "protobuf",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```protobuf
syntax = "proto3";

package aut_usuario;

// Nullable columns: ID, PESSOA_ID, EMAIL, SENHA_HASH
message AutUsuario {
    int32 id = 1;
    // FK → PEP_PESSOA.ID
    int32 pessoa_id = 2;
    string email = 3;
    string senha_hash = 4;
    bool ativo = 5;
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
  "tables": ["AUT_USUARIO"],
  "language": "openapi",
  "database": "financeiro",
  "include_fks": true
}
```

**Result:**
```yaml
AutUsuario:
  type: object
  description: Schema for table AUT_USUARIO
  required:
    - ativo
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
    pessoaId:
      type: integer
      format: int32
      nullable: true
      x-foreign-key: PEP_PESSOA.ID
    email:
      type: string
      nullable: true
    senhaHash:
      type: string
      nullable: true
    ativo:
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
  "tables": ["PEP_PESSOA"],
  "language": "lazarus",
  "database": "financeiro"
}
```

**Result:**
```lfm
object FormPepPessoa: TFormManutencao
  Left = 0
  Top = 0
  Caption = 'FormPepPessoa'
  ClientHeight = 400
  ClientWidth = 600
  LCLVersion = '3.0.0.0'
  object FBDataSetPepPessoa: TFBDataSet
    Left = 24
    Top = 24
    Connected = False
    SQL.Strings = (
      'SELECT * FROM PEP_PESSOA'
    )
  end
  object DataSourcePepPessoa: TDataSource
    Left = 112
    Top = 24
    DataSet = FBDataSetPepPessoa
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
    DataSource = DataSourcePepPessoa
    DataField = 'ID'
    TabOrder = 0
  end
  object Label_IS_MATRIZ: TLabel
    Left = 24
    Top = 160
    Width = 150
    Height = 15
    Caption = 'IS_MATRIZ:'
    ParentColor = False
  end
  object TDBCheckBox_IS_MATRIZ: TDBCheckBox
    Left = 24
    Top = 181
    Width = 150
    Height = 23
    DataSource = DataSourcePepPessoa
    DataField = 'IS_MATRIZ'
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
    DataSource = DataSourcePepPessoa
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
  "tables": ["PEP_PESSOA"],
  "language": "svelte",
  "database": "financeiro"
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
	let searchPAI_ID = $state('');
	let searchTIPO_ID = $state('');

	const columns: TableCol[] = [
		{ key: 'id', title: 'Id', sortable: true },
		{ key: 'tipoId', title: 'TipoId' },
		{ key: 'paiId', title: 'PaiId' },
		{ key: 'isMatriz', title: 'IsMatriz' },
		{ key: 'ativo', title: 'Ativo' },
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
	title="PepPessoa"
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
			id="pep_pessoa"
			caption="PepPessoa"
			{data}
			{columns}
			{actions}
			rowKey="id"
			csvFileName="pep_pessoa.csv"
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
	let tipoId = $state<number>(0);
	let paiId = $state<number>(0);
	let isMatriz = $state<boolean>(false);
	let ativo = $state<boolean>(false);
	let createdAt = $state<string>('');
	// ...

	function save() { /* TODO */ }
	function cancel() { /* TODO */ }
</script>

<form>
	<NumericField label="Id" bind:value={id} />
	<NumericField label="TipoId" bind:value={tipoId} />
	<NumericField label="PaiId" bind:value={paiId} />
	<SwitchField label="IsMatriz" bind:value={isMatriz} />
	<SwitchField label="Ativo" bind:value={ativo} />
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
- `language` (required): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `lazarus`, or `svelte`
- `database` (optional): Database name

**Example (Java):**
```json
{
  "views": ["VW_USER"],
  "language": "java",
  "database": "financeiro"
}
```

**Result:**
```java
@Data
public class VwUser {
    private Long id;
    private String email;
    private String senhaHash;
    private Boolean ativo;
    private Long pessoaId;
    private Long tipoId;
    private Boolean isMatriz;
    private String nome;
    private java.time.LocalDateTime createdAt;
    private java.time.LocalDateTime updatedAt;
    private String createdBy;
    private String updatedBy;
}
```

**Example (Lazarus/Delphi):**
```json
{
  "views": ["VW_USER"],
  "language": "lazarus",
  "database": "financeiro"
}
```

**Result:**
```lfm
object FormVwUser: TFormConsulta
  Left = 0
  Top = 0
  Caption = 'FormVwUser'
  ClientHeight = 400
  ClientWidth = 600
  LCLVersion = '3.0.0.0'
  object FBDataSetVwUser: TFBDataSet
    Left = 24
    Top = 24
    Connected = False
    SQL.Strings = (
      'SELECT * FROM VW_USER'
    )
  end
  object DataSourceVwUser: TDataSource
    Left = 112
    Top = 24
    DataSet = FBDataSetVwUser
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
    DataSource = DataSourceVwUser
    DataField = 'ID'
    TabOrder = 0
  end
  object Label_ATIVO: TLabel
    Left = 24
    Top = 160
    Width = 150
    Height = 15
    Caption = 'ATIVO:'
    ParentColor = False
  end
  object TDBCheckBox_ATIVO: TDBCheckBox
    Left = 24
    Top = 181
    Width = 150
    Height = 23
    DataSource = DataSourceVwUser
    DataField = 'ATIVO'
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

## Project Structure

```
firebird-5-mcp/
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
└── README.md
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

**Result:** ~1.2MB static binary (vs ~8MB for the Go version)

## License

MIT
