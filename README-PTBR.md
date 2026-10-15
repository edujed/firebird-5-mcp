# firebird-5-mcp

Um servidor MCP (Model Context Protocol) leve e em Rust puro para bancos de dados Firebird 5. Sem dependências externas — sem `isql`, sem DLLs, sem arquivos `.so`. Apenas um binário estáático.

## Funcionalidades

### Query & Dados
- `firebird_query` — Executa qualquer instrução SQL (SELECT, INSERT, UPDATE, DELETE, DDL)
- `firebird_count` — Conta linhas em uma tabela (com WHERE opcional)
- `firebird_sample` — Amostra rápida de dados da tabela
- `firebird_insert_batch` — Insere múltiplas linhas em uma única transação
- `firebird_run_script` — Executa scripts SQL com múltiplas instruções
- `firebird_execute_immediate` — Executa instruções com blocos SET TERM (procedimentos, funções, geradores)

### Schema & Estrutura
- `firebird_describe_table` — Schema completo de uma tabela (colunas, índices, constraints, triggers, comentários)
- `firebird_describe_database` — Visão completa do banco com seção Health (gstat-like) + diagrama ER Mermaid
- `firebird_describe` — Describe multi-objeto (tabelas, views, procedimentos, funções, triggers, sequências, domains)
- `firebird_list` — Lista tabelas, views, procedimentos, funções, domains, triggers, índices, sequências
- `firebird_find_column` — Busca colunas por padrão de nome (LIKE) em todas as tabelas/views
- `firebird_check_duplicates` — Verifica valores duplicados antes de criar índice/constraint único

### DDL & Avançado
- `firebird_create_database` — Cria um novo arquivo de banco Firebird
- `firebird_create_trigger` — Cria triggers BEFORE/AFTER INSERT/UPDATE/DELETE
- `firebird_create_procedure` — Cria stored procedures (sem isql)
- `firebird_create_function` — Cria funções definidas pelo usuário
- `firebird_create_sequence` — Cria sequências (geradores)
- `firebird_alter_column_type` — Altera tipo de coluna usando domains
- `firebird_drop` — Remove tabelas, views, domains, triggers, procedimentos, funções, índices, sequências

### Geração de Código
- `firebird_generate_dev` — Gera código de desenvolvimento a partir de schemas de tabelas:
  - **Java** (JPA + Lombok)
  - **Go** (sqlx)
  - **Rust** (rsfbclient)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (schemas YAML)
  - **Lazarus/Delphi** (forms .lfm com TFBDataSet)
  - **Svelte 5** (Page + Form usando `@edujed/jedsvelted-ui`)
- `firebird_generate_view_dto` — Gera DTOs/records a partir de views (metadados resolvidos):
  - **Java** (Lombok @Data)
  - **Go** (tags sqlx)
  - **Rust** (FromRow)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (schemas YAML)
  - **Lazarus/Delphi** (forms .lfm com TFBDataSet)
  - **Svelte 5** (Page read-only usando `@edujed/jedsvelted-ui`)

### Descoberta
- `firebird_get_databases` — Lista bancos de dados configurados
- `firebird_metadata_extract` — Extração completa de DDL (substitui `isql -x`)
- `firebird_explain_plan` — Obtém plano de execução sem rodar a query (transação revertida)

## Pré-requisitos

- **Firebird 5** servidor rodando (ou qualquer versão 3+)
- **Rust** toolchain (apenas se compilando do source)
- Sem `isql`, sem ODBC, sem bibliotecas nativas

## Configuração

### 1. Configure seu servidor e bancos de dados

Crie um arquivo `config.json`:

```json
{
  "server": {
    "host": "localhost",
    "port": 3050,
    "user": "SYSDBA",
    "password": "masterkey",
    "allowed_paths": ["/caminho/para/bancos"],
    "databases": [
      {
        "name": "teste",
        "description": "Banco de desenvolvimento local",
        "path": "/caminho/para/bancos/teste.fdb"
      },
      {
        "name": "financeiro",
        "description": "Banco financeiro",
        "path": "/caminho/para/bancos/financeiro.fdb"
      }
    ],
    "default_database": "teste"
  }
}
```

| Campo | Descrição |
|-------|-----------|
| `host` | Hostname do servidor Firebird |
| `port` | Porta do servidor Firebird (padrão 3050) |
| `user` | Usuário do banco |
| `password` | Senha do banco |
| `allowed_paths` | Diretórios onde novos bancos podem ser criados |
| `databases` | Lista de bancos configurados |
| `default_database` | Banco a usar quando nenhum for especificado |

### 2. Build

Use o script de build a partir da raiz do projeto:

```bash
./scripts/build.sh rust          # Plataforma atual (binários em bin/)
./scripts/build.sh rust all      # Todas as plataformas suportadas
```

O script também empacota os binários em `dist/` como arquivos compactados:

- **Windows** → `.zip` (ex.: `firebird-5-mcp-windows-amd64.zip`)
- **Linux** → `.tar.gz` (ex.: `firebird-5-mcp-linux-amd64.tar.gz`)

```bash
./scripts/build.sh rust all
# Binários em: bin/
# Arquivos compactados em: dist/
```

Ou compile manualmente:

```bash
cd firebird-5-mcp
cargo build --release
# O binário estará em: firebird-5-mcp/target/release/firebird-5-mcp
```

**Notas sobre cross-compilação:**

| Plataforma | Target | Requisitos |
|---|---|---|
| linux/amd64 | `x86_64-unknown-linux-gnu` | Nenhum (nativo em x86_64) |
| linux/arm64 | `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu-gcc` (`sudo apt install gcc-aarch64-linux-gnu`) |
| windows/amd64 | `x86_64-pc-windows-gnu` | `x86_64-w64-mingw32-gcc` (`sudo apt install gcc-mingw-w64-x86-64`) |
| darwin/* | `*-apple-darwin` | [osxcross](https://github.com/tpoechtrager/osxcross) (pulado com aviso se não estiver instalado) |
| windows/arm64 | — | Não suportado (sem target GNU estável no Rust) |

### 3. Registre no Zed Editor

Adicione nas configurações do Zed (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "firebird-5-mcp": {
      "command": "/caminho/para/firebird-5-mcp",
      "enabled": true,
      "env": {
        "FIREBIRD_CONFIG": "/caminho/para/config.json"
      },
      "timeout": 30000
    }
  }
}
```

## Configuração

### `config.json`

Mesmo formato da versão Go. A versão Rust é totalmente compatível com configs existentes do Go.

### Entrada de banco de dados

```json
{
  "name": "teste",
  "description": "Banco de desenvolvimento local",
  "path": "/caminho/para/bancos/teste.fdb"
}
```

### Busca do arquivo de configuração

O servidor procura o `config.json` nesta ordem:
1. Variável de ambiente `FIREBIRD_CONFIG`
2. `./config.json` (diretório atual)
3. `~/.config/firebird-5-mcp/config.json`

### Variáveis de ambiente

| Variável | Descrição |
|----------|-----------|
| `FIREBIRD_CONFIG` | Caminho para config.json |

## Ferramentas

### `firebird_query`

Executa uma única instrução SQL. Para DML/DDL, o plano de execução é anexado ao resultado.

**Argumentos:**
- `sql` (obrigatório): A instrução SQL
- `database` (opcional): Nome do banco (padrão: `default_database`)

**Exemplo:**
```json
{
  "sql": "SELECT * FROM CLIENTES WHERE CIDADE = 'São Paulo'",
  "database": "teste"
}
```

**Resultado:**
```
### Query Results (3 rows)

Columns: ID (INT), NOME (VARCHAR), CIDADE (VARCHAR)

ID | NOME | CIDADE
--- | --- | ---
1 | Alice | São Paulo
2 | Bob | NULL
3 | Charlie | São Paulo
```

### `firebird_list`

Lista objetos do banco por tipo.

**Argumentos:**
- `type` (obrigatório): TABLES, PROCEDURES, FUNCTIONS, DOMAINS, TRIGGERS, INDEXES, ou SEQUENCES
- `database` (opcional): Nome do banco

**Exemplo:**
```json
{
  "type": "TABLES",
  "database": "teste"
}
```

**Resultado:**
```
### Tables and Views (5)

NAME | TYPE | COMMENT
--- | --- | ---
CLIENTES | TABLE | Registros de clientes
PEDIDOS | TABLE | Dados de pedidos
...
```

### `firebird_find_column`

Busca colunas por padrão de nome em todas as tabelas e views. Suporta `%` wildcards.

**Argumentos:**
- `column` (obrigatório): Padrão de nome da coluna (suporta `%`). Ex.: `EMAIL`, `%NOME%`, `ID`
- `database` (opcional): Nome do banco

**Exemplo:**
```json
{
  "column": "%EMAIL%",
  "database": "financeiro"
}
```

**Resultado:**
```
### Columns matching '%EMAIL%' (2 found)

TABLE | COLUMN | TYPE | NULL | VIEW
--- | --- | --- | --- | ---
AUT_USUARIO | EMAIL | VARCHAR(250) | NO |
VW_USER | EMAIL | VARCHAR(250) | YES | YES
```

### `firebird_check_duplicates`

Verifica valores duplicados em uma ou mais colunas. Use ANTES de criar índice/constraint único para validar integridade. Suporta chaves compostas.

**Argumentos:**
- `table` (obrigatório): Nome da tabela
- `columns` (obrigatório): Array de nomes de colunas (suporta chaves compostas)
- `limit` (opcional): Máximo de combinações duplicadas (default: 10)
- `database` (opcional): Nome do banco

**Exemplo:**
```json
{
  "table": "PEP_PESSOA",
  "columns": ["TIPO_ID"],
  "database": "financeiro"
}
```

**Resultado (com duplicatas):**
```
### Duplicate Check: PEP_PESSOA (TIPO_ID)

⚠️ 1 duplicate combination(s) found:

TIPO_ID | DUPLICATED_COUNT
--- | ---
136323073 | 5

#### Full rows for first duplicate (TIPO_ID = 136323073):

ID | TIPO_ID | PAI_ID | IS_MATRIZ | ATIVO | ...
--- | --- | --- | --- | --- | ---
2 | 136323073 | NULL | false | true | ...
3 | 136323073 | 2 | true | true | ...
4 | 136323073 | NULL | false | true | ...
5 | 136323073 | 4 | true | true | ...
6 | 136323073 | 4 | false | true | ...

💡 Fix duplicates before creating the unique index/constraint.
```

**Resultado (sem duplicatas):**
```
### Duplicate Check: PEP_PESSOA (ID)

✅ No duplicates found. Safe to create unique index/constraint.
```

### `firebird_describe_table`

Mostra o schema completo de uma tabela ou view.

**Argumentos:**
- `table_name` (obrigatório): Nome da tabela ou view
- `database` (opcional): Nome do banco

**Resultado:**
```
### Table: CLIENTES

#### Columns

NAME | TYPE | NULL | DEFAULT | POSITION
--- | --- | --- | --- | ---
ID | INTEGER | NO | - | 1
NOME | VARCHAR(100) | NO | - | 2
EMAIL | VARCHAR(255) | YES | - | 3
CRIADO_EM | TIMESTAMP | NO | CURRENT_TIMESTAMP | 4

#### Indexes

NAME | UNIQUE | TYPE | COLUMNS
--- | --- | --- | ---
PK_CLIENTES | YES | 0 | ID

#### Constraints

NAME | TYPE
--- | ---
PK_CLIENTES | PRIMARY KEY
```

### `firebird_describe_database`

Fornece uma visão completa da estrutura do banco com uma seção Health (info gstat-like) e um diagrama ER Mermaid.

**Argumentos:**
- `database` (opcional): Nome do banco
- `include_counts` (opcional): Incluir contagem de linhas (pode ser lento)
- `include_describes` (opcional): Incluir detalhes das colunas

**Resultado:**
```
### Database: financeiro

#### Health

- **Path:** /database/financeiro.fdb
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

**AUT_ROLE**
- ID (INTEGER, PK)
...

#### Relationships (Mermaid)

```mermaid
erDiagram
    PEP_PESSOA ||--o{ AUT_USUARIO : "FK_AUT_USR_PESSOA"
    ...
```
```

**Campos da seção Health:**
| Campo | Descrição |
|-------|-----------|
| Path | Caminho do arquivo do banco |
| ODS Version | Versão On-Disk Structure (13.1 = Firebird 5) |
| Page Size | Tamanho da página em bytes |
| File Size | Total de páginas e tamanho estimado em MB |
| Dialect | Dialeto SQL (3 = ANSI) |
| Mode | Modo de shutdown: Online (Normal), Multi-user, Single-user, Full |
| Read-Only | Se o banco está somente leitura |
| Forced Writes | Se forced writes está habilitado |
| Active Connections | Número de conexões de usuário ativas |
| Oldest Transaction | ID da transação mais antiga (ativa e próxima) |
| Buffer Pool | Número de páginas do buffer pool |

### `firebird_describe`

Describe multi-objeto — aceita qualquer mistura de tabelas, views, procedimentos, funções, triggers, sequências e domains.

**Argumentos:**
- `objects` (obrigatório): Array de nomes de objetos
- `database` (opcional): Nome do banco

**Exemplo:**
```json
{
  "objects": ["SAVE_USER", "AUT_USUARIO", "D_BOOL"],
  "database": "financeiro"
}
```

**Resultado (procedimento com parâmetros):**
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

Gera código de desenvolvimento a partir de schemas de tabelas.

**Argumentos:**
- `tables` (obrigatório): Array de nomes de tabelas (máx. 20)
- `language` (obrigatório): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `lazarus`, ou `svelte`
- `database` (opcional): Nome do banco
- `include_fks` (opcional): Se true, gera relações nomeadas de FK (Java `@ManyToOne`) ou comentários de FK

**Exemplo (Go):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "go",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Java):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "java",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Rust):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "rust",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (TypeScript):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "typescript",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Protobuf):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "protobuf",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (OpenAPI):**
```json
{
  "tables": ["AUT_USUARIO"],
  "language": "openapi",
  "database": "financeiro",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Lazarus/Delphi):**
```json
{
  "tables": ["PEP_PESSOA"],
  "language": "lazarus",
  "database": "financeiro"
}
```

**Resultado:**
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

**Mapeamento de componentes:**
| Tipo Firebird | Componente Lazarus | Largura |
|---------------|-------------------|---------|
| INTEGER/BIGINT/SMALLINT | TDBEdit | 120 |
| VARCHAR/CHAR | TDBEdit | 80/250/450 (dinâmica) |
| NUMERIC/DECIMAL | TDBEdit | 120 |
| BOOLEAN | TDBCheckBox | 150 |
| DATE/TIMESTAMP | TDBDateTimePicker | 150 |
| BLOB SUB_TYPE TEXT | TDBMemo | 450 |
| BLOB SUB_TYPE BINARY | TDBEdit | 250 |

**Exemplo (Svelte 5):**
```json
{
  "tables": ["PEP_PESSOA"],
  "language": "svelte",
  "database": "financeiro"
}
```

**Resultado:** Gera dois componentes:

1. **Page** — `PageShell` + `Table` com busca por PK e FKs:
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

	// Filtros de busca (PK + FKs)
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

	async function search() { /* TODO: chamada API */ }
	function clear() { /* reseta campos de busca */ }
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
			<!-- campos de busca FK -->
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

2. **Form** — Campos com `$state` (sem lógica de eventos):
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

**Mapeamento de campos:**
| Tipo Firebird | Componente Svelte | Tipo TS |
|---------------|------------------|--------|
| BOOLEAN | SwitchField | boolean |
| DATE/TIMESTAMP | DateField | string |
| INTEGER/BIGINT/SMALLINT/NUMERIC/DECIMAL | NumericField | number |
| VARCHAR/CHAR/BLOB | EditField | string |

### `firebird_generate_view_dto`

Gera DTOs/records a partir de views usando metadados resolvidos (executa `SELECT FIRST 1 * FROM view` para obter tipos de colunas precisos). O código gerado é somente leitura (sem anotações de entidade, sem FKs).

**Argumentos:**
- `views` (obrigatório): Array de nomes de views (máx. 20)
- `language` (obrigatório): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `lazarus`, ou `svelte`
- `database` (opcional): Nome do banco

**Exemplo (Java):**
```json
{
  "views": ["VW_USER"],
  "language": "java",
  "database": "financeiro"
}
```

**Resultado:**
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

**Exemplo (Lazarus/Delphi):**
```json
{
  "views": ["VW_USER"],
  "language": "lazarus",
  "database": "financeiro"
}
```

**Resultado:**
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

Obtém o plano de execução de uma instrução SQL sem efetivamente executá-la. A instrução é executada dentro de uma transação revertida.

**Argumentos:**
- `sql` (obrigatório): A instrução SQL
- `database` (opcional): Nome do banco

**Resultado:**
```
Execution plan (the statement was executed inside a rolled-back transaction):
PLAN:
  SORT
    TABLE: CLIENTES (INDEX: PK_CLIENTES)
```

### `firebird_metadata_extract`

Extrai metadados completos do banco (DDL) usando tabelas de sistema. Substitui `isql -x`.

**Argumentos:**
- `database` (opcional): Nome do banco
- `objects` (opcional): Array de nomes de objetos para filtrar (ex: `["CLIENTES", "PEDIDOS"]`)

**Resultado:**
```sql
CREATE TABLE CLIENTES (
    ID INTEGER NOT NULL,
    NOME VARCHAR(100) NOT NULL,
    EMAIL VARCHAR(255),
    CRIADO_EM TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT PK_CLIENTES PRIMARY KEY (ID)
);

CREATE INDEX IDX_CLIENTES_EMAIL ON CLIENTES (EMAIL);
...
```

## Estrutura do Projeto

```
firebird-5-mcp/
├── Cargo.toml              # Manifesto do workspace
├── crates/
│   ├── mcp-core/           # Biblioteca reutilizável (firebird-5-mcp-core)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs      # API pública
│   │       ├── config.rs   # Carregamento de config
│   │       ├── client.rs   # Wrapper de conexão
│   │       ├── query.rs    # Execução de SQL
│   │       ├── schema.rs   # Introspecção de schema
│   │       ├── ddl.rs      # Operações DDL
│   │       ├── explain.rs  # Planos de execução
│   │       ├── metadata.rs # Extração de DDL
│   │       ├── generator.rs# Geração de código
│   │       └── types.rs    # Formatação de tipos
│   └── mcp-bin/            # Binário do servidor MCP (firebird-5-mcp)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs     # Loop JSON-RPC stdio
│           └── tools/
│               ├── mod.rs          # Dispatch
│               ├── common.rs       # Helpers compartilhados
│               ├── definitions.rs  # Schemas das tools
│               ├── query_tools.rs  # Tools de query
│               ├── schema_tools.rs # Tools de schema
│               ├── ddl_tools.rs    # Tools de DDL
│               └── generator_tools.rs # Geração de código
└── README.md
```

### Design

O projeto é dividido em dois crates:

1. **`firebird-5-mcp-core`** — Uma biblioteca reutilizável que fornece:
   - Gerenciamento de conexões com o banco
   - Execução de queries SQL
   - Introspecção de schema
   - Operações DDL
   - Geração de código

   Esta biblioteca pode ser usada em outros projetos Rust (ex: apps desktop, ferramentas CLI) sem a camada MCP.

2. **`firebird-5-mcp`** — O binário do servidor MCP que:
   - Implementa o protocolo MCP (JSON-RPC sobre stdio)
   - Expõe a funcionalidade da biblioteca core como tools MCP
   - Lida com configuração e reporte de erros

### Estendendo com novos motores de banco

A biblioteca core é projetada para ser agnóstica de banco de dados. Para adicionar suporte a um novo motor:

1. Crie um novo crate (ex: `postgres-mcp-core`)
2. Implemente as mesmas traits/interfaces
3. Crie um novo binário de servidor MCP (ex: `postgres-mcp`)

## Build Otimizado para Deploy

O perfil de release é otimizado para tamanho mínimo de binário e máxima performance:

```toml
[profile.release]
opt-level = 3            # Otimização máxima
codegen-units = 1        # Melhor codegen (build mais lento, melhor runtime)
lto = "fat"              # Link-time optimization agressivo
panic = "abort"           # Binário menor (sem unwinding)
strip = true              # Remove símbolos do binário
debug = false             # Sem debug info
```

**Resultado:** ~1.2MB binário estático (vs ~8MB para a versão Go)

## Créditos

- [rsfbclient](https://crates.io/crates/rsfbclient) — Driver de cliente Firebird em Rust puro (sem bibliotecas nativas)
- [jedSvelted-ui](https://github.com/edujed/jedSvelted-ui) — Biblioteca de componentes Svelte 5 usada pelo gerador de código Svelte (`@edujed/jedsvelted-ui`)
- [Model Context Protocol](https://modelcontextprotocol.io) — A especificação MCP

## Licença

MIT — veja [LICENSE](LICENSE).
