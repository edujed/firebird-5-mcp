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
  - **GraphQL** (schema + resolvers Node.js)
  - **Lazarus/Delphi** (forms .lfm com TFBDataSet)
  - **Svelte 5** (Page + Form usando `@edujed/jedsvelted-ui`)
- `firebird_generate_view_dto` — Gera DTOs/records a partir de views (metadados resolvidos):
  - **Java** (Lombok @Data)
  - **Go** (tags sqlx)
  - **Rust** (FromRow)
  - **TypeScript** (interfaces)
  - **Protobuf** (Connect-RPC/gRPC)
  - **OpenAPI 3.2.0** (schemas YAML)
  - **GraphQL** (schema + resolvers Node.js, apenas Query)
  - **Lazarus/Delphi** (forms .lfm com TFBDataSet)
  - **Svelte 5** (Page read-only usando `@edujed/jedsvelted-ui`)

### Descoberta
- `firebird_get_databases` — Lista bancos de dados configurados
- `firebird_metadata_extract` — Extração completa de DDL (substitui `isql -x`)
- `firebird_explain_plan` — Obtém plano de execução sem rodar a query (transação revertida)

### Introspecção
- `firebird_table_stats` — Contagem de linhas por tabela (tabela única ou todas)
- `firebird_users` — Lista todos os usuários com acesso ao banco (criadores + grants)
- `firebird_grants` — Mostra GRANTs de uma tabela ou de todas
- `firebird_blob_info` — Lista colunas BLOB com subtipo (TEXT/BINARY)
- `firebird_index_usage` — Mostra todos os índices com colunas, unicidade e tipo

> **⚠️ Não recomendado para bancos de produção.**
> Este MCP foi projetado para fluxos de desenvolvimento e análise. Tools de geração de código,
> operações DDL e instruções de refactoring podem ser destrutivas. Para produção,
> use uma ferramenta de administração dedicada (ex.: `isql`, `fbguard`, ou a suíte preferida do seu DBA).

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
./scripts/build.sh              # Plataforma atual (binários em bin/)
./scripts/build.sh all          # Todas as plataformas suportadas
```

O script também empacota os binários em `dist/` como arquivos compactados:

- **Windows** → `.zip` (ex.: `firebird-5-mcp-windows-amd64.zip`)
- **Linux** → `.tar.gz` (ex.: `firebird-5-mcp-linux-amd64.tar.gz`)

```bash
./scripts/build.sh all
# Binários em: bin/
# Arquivos compactados em: dist/
```

Ou compile manualmente:

```bash
cargo build --release
# O binário estará em: target/release/firebird-5-mcp
```

**Notas sobre cross-compilação:**

| Plataforma | Target | Requisitos |
|---|---|---|
| linux/amd64 | `x86_64-unknown-linux-gnu` | Nenhum (nativo em x86_64) |
| linux/arm64 | `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu-gcc` (`sudo apt install gcc-aarch64-linux-gnu`) |
| windows/amd64 | `x86_64-pc-windows-gnu` | `x86_64-w64-mingw32-gcc` (`sudo apt install gcc-mingw-w64-x86-64`) |
| darwin/* | `*-apple-darwin` | [osxcross](https://github.com/tpoechtrager/osxcross) (pulado com aviso se não estiver instalado) |
| windows/arm64 | — | Não suportado (sem target GNU estável no Rust) |

### 3. Registre nos Editores

#### 3.1. Zed Editor

Adicione nas configurações do Zed (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "firebird-5-mcp": {
      "command": "/caminho/para/firebird-5-mcp",
      "enabled": true,
      "timeout": 30000
    }
  }
}
```

#### 3.2. VSCode

No VSCode, abra a paleta de comandos ('Ctrl+Shift+P') e execute: 'MCP: Add Server'.
Escolha 'Command (stdio)' e insira o binário (descompactado) com o caminho (exemplo: `/caminho/para/firebird-5-mcp`)
Informe o nome do servidor `firebird-5-mcp` e escolha entre 'personal/user' ou 'project/workspace'.
Confirme que você 'trust in the server' (quando perguntado) e habilite as ferramentas em 'Configure Tools'.

Se preferir, é possível configurar pelo `mpc-config.json` (como `/usr/folder/.copilot/mpc-config.json`)
e digitar

```json
{
	"mcpServers": {
		"firebird-5-mcp": {
			"type": "local",
			"command": "/caminho/para/firebird-5-mcp",
			"args": [],
			"tools": ["*"]	
		}
	}
}
```

#### 3.3. Cursor

Crie um arquivo `.cursor/mcp.json` na raiz do projeto (ou `~/.cursor/mcp.json` para global):

```json
{
  "mcpServers": {
    "firebird-5-mcp": {
      "command": "/caminho/para/firebird-5-mcp",
      "args": []
    }
  }
}
```

#### 3.4. JetBrains (IntelliJ, GoLand, WebStorm, CLion)

Nas configurações do AI Assistant, adicione um novo servidor MCP:

- **Command:** `/caminho/para/firebird-5-mcp`

Ou edite o arquivo de configuração MCP (`~/.jetbrains/mcp.json` ou `.mcp.json` no projeto):

```json
{
  "mcpServers": {
    "firebird-5-mcp": {
      "command": "/caminho/para/firebird-5-mcp",
      "args": []
    }
  }
}
```

> **Nota:** O binário resolve o `config.json` automaticamente de `~/.firebird-mcp/config.json` (ou via variáveis `FIREBIRD_CONFIG` / `FIREBIRD_MCP_HOME`). Não é necessário definir variáveis de ambiente na configuração do editor, a menos que use um caminho não-padrão.

## Uso pela Linha de Comando (Sem IDE)

O servidor MCP fala JSON-RPC via stdio. Você pode chamar qualquer ferramenta diretamente do shell — útil para scripts, pipelines de CI, ou IDEs sem suporte a MCP (ex.: Lazarus).

### Scripts de ajuda

Scripts prontos para uso estão em [`examples/`](examples/):

| Script | Plataforma | Uso |
|--------|------------|-----|
| [`mcp-call.sh`](examples/mcp-call.sh) | Linux/macOS | `./mcp-call.sh <ferramenta> '<json_args>'` |
| [`mcp-call.bat`](examples/mcp-call.bat) | Windows | `mcp-call.bat <ferramenta> <json_args>` |

**Exemplos (Linux/macOS):**

```bash
# Listar tabelas
./examples/mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Consultar dados
./examples/mcp-call.sh firebird_query '{"sql":"SELECT * FROM USERS","database":"test-mcp"}'

# Descrever uma tabela
./examples/mcp-call.sh firebird_describe_table '{"table_name":"USERS","database":"test-mcp"}'

# Gerar código (OpenAPI)
./examples/mcp-call.sh firebird_generate_dev '{"tables":["USERS"],"language":"openapi","database":"test-mcp"}'
```

Defina `MCP_BINARY` no script (ou como variável de ambiente) para apontar para o binário. O arquivo de configuração é resolvido automaticamente pelo binário (`~/.firebird-mcp/config.json` por padrão).

### Como funciona

1. A requisição JSON-RPC é enviada ao **stdin** do servidor via pipe
2. O servidor processa a chamada e escreve a resposta JSON-RPC no **stdout**
3. Quando o stdin atinge EOF, o servidor encerra
4. O `python3` extrai o campo `result.content[0].text` da resposta

> **Nota:** O `timeout 10` evita travamentos se o servidor não encerrar. O `2>/dev/null` suprime o stderr (diagnósticos).

### Integração com Lazarus / Free Pascal

Para IDEs sem suporte a MCP (como o Lazarus), você pode chamar o servidor MCP diretamente do código Pascal. O exemplo abaixo usa **pipes POSIX puros** (`fork`/`exec`/`read`/`write`) em vez de `TProcess`, porque a API do `TProcess` no FPC 3.2.2 é limitada (sem `WaitFor`, `OutputLines` ou `CloseInput`).

Um exemplo completo e testado está em [`examples/freePascal/`](examples/freePascal/).

**Compilar e rodar o programa de teste:**

```bash
cd examples/freePascal
fpc -otest_mcp test_mcp.pas
./test_mcp
```

> **Nota:** Cada `MCPCall` inicia um novo processo. Para chamadas de alta frequência, considere manter uma conexão persistente (escrever no stdin, ler do stdout em uma thread) — mas para workflows de desenvolvimento típicos, a abordagem spawn-per-call é simples e suficiente.

## Configuração

### `config.json`

O arquivo de configuração define a conexão com o servidor Firebird e a lista de bancos de dados disponíveis para as ferramentas MCP.

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
| `user` | Nome do usuário do banco |
| `password` | Senha do banco |
| `allowed_paths` | Diretórios onde novos bancos podem ser criados |
| `databases` | Lista de bancos configurados |
| `default_database` | Banco a usar quando nenhum for especificado |

### Entrada de banco de dados

Cada entrada no array `databases`:

```json
{
  "name": "teste",
  "description": "Banco de desenvolvimento local",
  "path": "/caminho/para/bancos/teste.fdb"
}
```

### Busca do arquivo de configuração

O servidor procura o `config.json` nesta ordem:
1. Variável de ambiente `FIREBIRD_CONFIG` (caminho completo)
2. `./config.json` (diretório de trabalho atual)
3. `~/.firebird-mcp/config.json` (padrão)

O diretório base pode ser sobrescrito com `FIREBIRD_MCP_HOME` (ex.: para testes ou instalações não-padrão).

### Variáveis de ambiente

| Variável | Descrição |
|----------|-----------|
| `FIREBIRD_CONFIG` | Caminho completo para `config.json` (maior prioridade) |
| `FIREBIRD_MCP_HOME` | Sobrescreve o diretório base (padrão: `~/.firebird-mcp`) |

## Ferramentas

### `firebird_query`

Executa uma única instrução SQL. Para DML/DDL, o plano de execução é anexado ao resultado.

**Argumentos:**
- `sql` (obrigatório): A instrução SQL
- `database` (opcional): Nome do banco (padrão: `default_database`)

**Exemplo:**
```json
{
  "sql": "SELECT * FROM CLIENTS WHERE CITY = 'New York'",
  "database": "shop"
}
```

**Resultado:**
```
### Query Results (3 rows)

Columns: ID (INT), NOME (VARCHAR), CIDADE (VARCHAR)

ID | NOME | CIDADE
--- | --- | ---
1 | Alice | New York
2 | Bob | NULL
3 | Charlie | New York
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
  "database": "shop"
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
  "database": "shop"
}
```

**Resultado:**
```
### Columns matching '%EMAIL%' (2 found)

TABLE | COLUMN | TYPE | NULL | VIEW
--- | --- | --- | --- | ---
USERS | EMAIL | VARCHAR(250) | NO |
VW_ACTIVE_USERS | EMAIL | VARCHAR(250) | YES | YES
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
  "table": "CUSTOMERS",
  "columns": ["TYPE_ID"],
  "database": "shop"
}
```

**Resultado (com duplicatas):**
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

**Resultado (sem duplicatas):**
```
### Duplicate Check: CUSTOMERS (ID)

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
  "objects": ["SAVE_USER", "USERS", "D_BOOL"],
  "database": "shop"
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
- `language` (obrigatório): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `graphql`, `lazarus`, ou `svelte`
- `database` (opcional): Nome do banco
- `include_fks` (opcional): Se true, gera relações nomeadas de FK (Java `@ManyToOne`) ou comentários de FK

**Exemplo (Go):**
```json
{
  "tables": ["USERS"],
  "language": "go",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Java):**
```json
{
  "tables": ["USERS"],
  "language": "java",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Rust):**
```json
{
  "tables": ["USERS"],
  "language": "rust",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (TypeScript):**
```json
{
  "tables": ["USERS"],
  "language": "typescript",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Protobuf):**
```json
{
  "tables": ["USERS"],
  "language": "protobuf",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (OpenAPI):**
```json
{
  "tables": ["USERS"],
  "language": "openapi",
  "database": "shop",
  "include_fks": true
}
```

**Resultado:**
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

**Exemplo (Lazarus/Delphi):**
```json
{
  "tables": ["CUSTOMERS"],
  "language": "lazarus",
  "database": "shop"
}
```

**Resultado:**
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
  "tables": ["CUSTOMERS"],
  "language": "svelte",
  "database": "shop"
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

	async function search() { /* TODO: chamada API */ }
	function clear() { /* reseta campos de busca */ }
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
			<!-- campos de busca FK -->
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
- `language` (obrigatório): `java`, `go`, `rust`, `typescript`, `protobuf`, `openapi`, `graphql`, `lazarus`, ou `svelte`
- `database` (opcional): Nome do banco

**Exemplo (Java):**
```json
{
  "views": ["VW_ACTIVE_USERS"],
  "language": "java",
  "database": "shop"
}
```

**Resultado:**
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

**Exemplo (Lazarus/Delphi):**
```json
{
  "views": ["VW_ACTIVE_USERS"],
  "language": "lazarus",
  "database": "shop"
}
```

**Resultado:**
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

### `firebird_table_stats`

Retorna contagem de linhas por tabela. Para uma tabela específica, usa `SELECT COUNT(*) FROM <tabela>`. Para todas as tabelas, usa um `EXECUTE BLOCK` com `EXECUTE STATEMENT` para contar dinamicamente cada tabela.

**Argumentos:**
- `table` (opcional): Nome da tabela. Omitir para obter stats de todas as tabelas.
- `database` (opcional): Nome do banco de dados

**Resultado:**
```
## Table Stats (all tables)

TABLE_NAME | TOTAL
--- | ---
CLIENTES | 1542
PEDIDOS | 8931
PRODUTOS | 234
```

> **Nota:** `COUNT(*)` em tabelas muito grandes pode ser lento. Destinado a desenvolvimento/análise.

### `firebird_users`

Lista todos os usuários com acesso ao banco, combinando `RDB$DB_CREATORS` (donos do banco) com `RDB$USER_PRIVILEGES` (usuários com grants explícitos).

**Argumentos:**
- `database` (opcional): Nome do banco de dados

**Resultado:**
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

Mostra permissões GRANT para uma tabela específica ou para todas as tabelas, usando `RDB$USER_PRIVILEGES`.

**Argumentos:**
- `table` (opcional): Nome da tabela. Omitir para mostrar grants de todas as tabelas.
- `database` (opcional): Nome do banco de dados

**Resultado:**
```
## Grants for CLIENTES

RELATION_NAME | GRANTEE | PRIVILEGE | GRANT_OPTION
--- | --- | --- | ---
CLIENTES | APP_USER | SELECT | 
CLIENTES | APP_USER | INSERT | 
CLIENTES | APP_USER | UPDATE | 
```

### `firebird_blob_info`

Lista todas as colunas BLOB no banco (ou em uma tabela específica): tabela, coluna, subtipo (TEXT/BINARY) e comprimento do campo.

**Argumentos:**
- `table` (opcional): Nome da tabela. Omitir para listar colunas BLOB de todas as tabelas.
- `database` (opcional): Nome do banco de dados

**Resultado:**
```
## BLOB Columns (all tables)

RELATION_NAME | FIELD_NAME | FIELD_TYPE | SUB_TYPE | LENGTH | BLOB_TYPE
--- | --- | --- | --- | --- | ---
DOCUMENTOS | CONTEUDO | 261 | 1 | 0 | TEXT
FATURAS | PDF_DADOS | 261 | 2 | 0 | BINARY
```

### `firebird_index_usage`

Mostra todos os índices no banco (ou em uma tabela específica): nome do índice, tabela, unicidade, tipo e colunas.

**Argumentos:**
- `table` (opcional): Nome da tabela. Omitir para mostrar índices de todas as tabelas.
- `database` (opcional): Nome do banco de dados

**Resultado:**
```
## Indexes (all tables)

INDEX_NAME | RELATION_NAME | UNIQUE_FLAG | INDEX_TYPE | COLUMNS
--- | --- | --- | --- | ---
PK_CLIENTES | CLIENTES | 2 | 0 | ID
IDX_CLIENTES_EMAIL | CLIENTES | 1 | 0 | EMAIL
IDX_PEDIDOS_CLIENTE | PEDIDOS | 0 | 0 | CLIENTE_ID, CRIADO_EM
```

| UNIQUE_FLAG | Significado |
|---|---|
| 0 | Não único |
| 1 | Único |
| 2 | Chave primária |

## Estrutura do Projeto

```
firebird-5-mcp/             # Raiz do repositório
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
│   ├── mcp-tools/          # Handlers de tools compartilhados (agnóstico de transporte)
│   │   ├── Cargo.toml
│   │   └── src/
│   └── mcp-stdio-bin/      # Binário do servidor MCP stdio (firebird-5-mcp)
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
├── scripts/
│   ├── build.sh            # Build multi-plataforma
│   └── publish_new_tag.sh  # Helper de tag + release
├── tests/                  # Testes de integração (shell)
├── .github/workflows/
│   └── release.yml         # Pipeline CI/CD de release
├── README.md
├── CHANGELOG.md
└── LICENSE
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

**Resultado:** ~1.2MB binário estático

## Créditos

- [rsfbclient](https://crates.io/crates/rsfbclient) — Driver de cliente Firebird em Rust puro (sem bibliotecas nativas)
- [jedSvelted-ui](https://github.com/edujed/jedSvelted-ui) — Biblioteca de componentes Svelte 5 usada pelo gerador de código Svelte (`@edujed/jedsvelted-ui`)
- [Model Context Protocol](https://modelcontextprotocol.io) — A especificação MCP

## Licença

MIT — veja [LICENSE](LICENSE).
