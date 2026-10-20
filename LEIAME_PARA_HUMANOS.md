# firebird-5-mcp — Para Humanos

> **Você não precisa de um agente de IA para usar esta ferramenta.**
>
> O `firebird-5-mcp` é um servidor MCP para Firebird — mas também é um **CLI
> completo** que roda em cron jobs, pipelines de CI, scripts de shell e em
> qualquer lugar onde você automatize um banco de dados. Sem IDE. Sem LLM.
> Sem agente. Apenas um único binário estático que fala JSON-RPC via
> stdin/stdout.

Pense nele como **Firebird PowerToys**: o mesmo binário que alimenta agentes
de IA também te dá uma faca suíça para o trabalho diário com banco de dados —
exportar tabelas para o DataLake, consolidar dados de outras ferramentas,
executar procedures em lote por agendamento, snapshot do schema para
versionamento.

---

## Por que usar sem IA?

| Cenário | O que você ganha |
|---|---|
| **DataLake / BI** | Exporte qualquer tabela, view ou query SQL para Parquet, CSV, DBF ou JSON Lines — um comando, pronto para cron |
| **Consolidação de dados** | Importe Parquet/CSV/DBF/JSONL de outras ferramentas para o Firebird — crie ou acrescente, pule colunas |
| **Jobs em lote** | Execute procedures armazenadas ou scripts SQL em agendamento (fechamento mensal, ETL noturno) |
| **Health checks** | Estatísticas do banco, queries ativas, contagem de linhas — pipe para seu monitoramento/alertas |
| **Versionamento de schema** | Extraia o DDL completo, faça diff no git, rastreie cada mudança |
| **Pipelines de CI** | Rode scripts de teste contra um banco descartável, verifique contagens, desmonte |
| **Introspeção rápida** | Descreva uma tabela, encontre uma coluna, verifique duplicados — sem abrir GUI |

---

## Como funciona

O binário fala **JSON-RPC 2.0** via stdin/stdout:

1. Você faz pipe de um request JSON-RPC para o **stdin** do binário
2. Ele processa a chamada e escreve a resposta no **stdout**
3. Quando o stdin atinge **EOF**, o binário sai

Isso o torna perfeito para pipes, `xargs`, `cron` e qualquer workflow de shell:

```
echo '<json-rpc>' | firebird-5-mcp | jq '.result.content[0].text'
```

Uma linha entra, uma linha sai. Sem daemon. Sem porta. Sem estado.

---

## Começando

### 1. Obter o binário

**Opção A — GitHub Releases (maioria dos usuários):**

1. Vá em [Releases](https://github.com/your-org/firebird-5-mcp/releases)
2. Baixe o `.tgz` da sua plataforma (ex.: `firebird-5-mcp-linux-amd64.tgz`)
3. Extraia: `tar xzf firebird-5-mcp-linux-amd64.tgz`
4. Pronto — você tem o binário `firebird-5-mcp`

**Opção B — Compilar do source (desenvolvedores):**

```bash
git clone https://github.com/your-org/firebird-5-mcp.git
cd firebird-5-mcp
./scripts/build.sh          # compila → ~/.local/bin/firebird-5-mcp
```

### 2. Configurar

O binário carrega automaticamente `~/.firebird-mcp/config.json` (ou env
`FIREBIRD_CONFIG`, ou `./config.json`). Configuração mínima:

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
        "description": "Banco de teste MCP",
        "path": "/database/test-mcp.fdb"
      }
    ],
    "default_database": "test-mcp"
  }
}
```

> **Dica:** o diretório [`examples/test-mcp/`](examples/test-mcp/) contém um
> banco de teste completo (18 tabelas, 12 procedures, 10 funções, views,
> triggers) com script de criação em um passo:
>
> ```bash
> cd examples/test-mcp
> ./create_test_db.sh
> ```

### 3. Primeira chamada

O helper [`examples/mcp-call.sh`](examples/mcp-call.sh) encapsula o protocolo
JSON-RPC para você chamar qualquer tool com um comando:

```bash
# Listar tabelas
./examples/mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Consultar dados
./examples/mcp-call.sh firebird_query '{"sql":"SELECT * FROM USERS","database":"test-mcp"}'

# Descrever uma tabela
./examples/mcp-call.sh firebird_describe_table '{"table_name":"USERS","database":"test-mcp"}'

# Contar linhas
./examples/mcp-call.sh firebird_count '{"table":"USERS","database":"test-mcp"}'
```

> **Nota:** defina `MCP_BINARY` no script (ou como variável de ambiente)
> apontando para o seu binário. O arquivo de configuração é resolvido
> automaticamente.

---

## Receitas práticas

Todos os exemplos abaixo usam o banco `test-mcp`.

### Exportar uma tabela para o DataLake (Parquet)

```bash
# Exportar USERS para Parquet (compressão Snappy)
./examples/mcp-call.sh firebird_export '{
  "source": "USERS",
  "database": "test-mcp",
  "output_path": "/datalake/firebird/users.parquet",
  "format": "parquet",
  "compression": "snappy"
}'
```

**Cron job** — export noturno de todas as tabelas:

```bash
# /etc/cron.d/firebird-export
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"USERS","database":"test-mcp","output_path":"/datalake/firebird/users.parquet","format":"parquet"}'
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"DEPARTMENT","database":"test-mcp","output_path":"/datalake/firebird/department.parquet","format":"parquet"}'
0 2 * * * root /path/to/mcp-call.sh firebird_export '{"source":"APPROVAL","database":"test-mcp","output_path":"/datalake/firebird/approval.parquet","format":"parquet"}'
```

Você também pode exportar uma **query SQL** (não apenas uma tabela):

```bash
# Exportar uma view com JOIN
./examples/mcp-call.sh firebird_export '{
  "source": "SELECT U.NAME, D.NAME AS DEPT, B.CODE AS BRANCH FROM USERS U JOIN DEPARTMENT D ON D.ID = U.DEPARTMENT_ID JOIN BRANCH B ON B.ID = D.BRANCH_ID",
  "database": "test-mcp",
  "output_path": "/datalake/firebird/users_with_dept.csv",
  "format": "csv"
}'
```

### Importar dados de outra ferramenta

```bash
# Importar um CSV (a tabela deve já existir)
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/sales_report.csv",
  "database": "test-mcp",
  "table": "SALES",
  "mode": "append",
  "format": "csv"
}'

# Importar um Parquet, criando a tabela automaticamente
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/metrics.parquet",
  "database": "test-mcp",
  "table": "METRICS",
  "mode": "create",
  "format": "parquet"
}'

# Pular colunas que não quer
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/legacy.dbf",
  "database": "test-mcp",
  "table": "LEGACY_DATA",
  "mode": "create",
  "format": "dbf",
  "ignore_columns": ["_DELETED", "FILLER"]
}'

# Importar JSON Lines (um objeto JSON por linha) — tipos inferidos automaticamente
./examples/mcp-call.sh firebird_import '{
  "input_path": "/incoming/events.jsonl.zst",
  "database": "test-mcp",
  "table": "EVENTS",
  "mode": "create",
  "format": "jsonl"
}'
```

### Executar uma procedure em lote

O banco `test-mcp` inclui `SP_COUNT_APPROVALS` (conta aprovações por
usuário/status) e `SP_LIST_USERS` (lista usuários com departamento/filial).

```bash
# Executar procedure e obter resultados
./examples/mcp-call.sh firebird_query '{
  "sql": "SELECT * FROM SP_COUNT_APPROVALS(1,''PENDING'')",
  "database": "test-mcp"
}'
```

**Cron job** — executar procedure de fechamento mensal:

```bash
# /etc/cron.d/firebird-month-end
0 6 1 * * root /path/to/mcp-call.sh firebird_query '{"sql":"EXECUTE PROCEDURE SP_MONTH_END_CLOSE()","database":"test-mcp"}'
```

Ou executar um script SQL completo:

```bash
# Executar um arquivo de script em lote
./examples/mcp-call.sh firebird_run_script '{
  "database": "test-mcp",
  "path": "/scripts/fechamento_mensal.sql"
}'
```

### Health check + alertas

```bash
# Estatísticas do banco (tamanho, conexões, I/O, statements ativos)
./examples/mcp-call.sh firebird_database_stats '{"database":"test-mcp"}'

# Queries ativas (quem está rodando o quê agora)
./examples/mcp-call.sh firebird_query_history '{"database":"test-mcp","state":1}'

# Contagem de linhas de todas as tabelas
./examples/mcp-call.sh firebird_table_stats '{"database":"test-mcp"}'
```

**Cron job** — health check diário com alerta:

```bash
#!/bin/bash
# /usr/local/bin/firebird-healthcheck.sh
OUTPUT=$(./examples/mcp-call.sh firebird_database_stats '{"database":"test-mcp"}')
if echo "$OUTPUT" | grep -q "Read-Only: YES"; then
    echo "ALERTA: test-mcp está SOMENTE LEITURA!" | mail -s "Firebird Alerta" dba@example.com
fi
```

### Snapshot de schema para versionamento

```bash
# Extrair DDL completo (substitui isql -x)
./examples/mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}' > schema.sql

# Commitar no git
git add schema.sql
git diff --staged schema.sql   # revisar mudanças
git commit -m "schema: adicionar APPROVED_AT em APPROVAL"
```

**Cron job** — diff de schema noturno:

```bash
#!/bin/bash
# /usr/local/bin/firebird-schema-diff.sh
cd /repo/schema
./examples/mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}' > current.sql
if ! git diff --quiet current.sql; then
    echo "Schema alterado:" | mail -s "Firebird Schema Change" dba@example.com
    git diff current.sql | mail -s "Firebird Schema Diff" dba@example.com
fi
```

### Gerar código a partir do schema

```bash
# Gerar interfaces TypeScript
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS", "DEPARTMENT"],
  "language": "typescript",
  "database": "test-mcp"
}'

# Gerar spec OpenAPI
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS"],
  "language": "openapi",
  "database": "test-mcp"
}'

# Gerar entidades Java (JPA + Lombok)
./examples/mcp-call.sh firebird_generate_dev '{
  "tables": ["USERS", "APPROVAL"],
  "language": "java",
  "database": "test-mcp",
  "outputDirPath": "/tmp/gerado"
}'
```

---

## Tabela de poderes

| Tool | Uso humano |
|---|---|
| `firebird_export` | Publicar tabela/view/query no DataLake (Parquet/CSV/DBF/JSONL) |
| `firebird_import` | Consolidar dados de outras ferramentas no Firebird |
| `firebird_describe_file` | Inspecionar schema de arquivo recebido antes de importar |
| `firebird_query` | Executar qualquer SQL — SELECT, DML, DDL, procedures |
| `firebird_run_script` | Executar script SQL multi-instrução (jobs em lote, migrações) |
| `firebird_insert_batch` | Carga em massa de linhas em uma única transação |
| `firebird_database_stats` | Health check — tamanho, conexões, I/O, statements ativos |
| `firebird_query_history` | Ver quem está rodando o quê (queries ativas/ociosas/travadas) |
| `firebird_table_stats` | Contagem de linhas por tabela (monitoramento, capacidade) |
| `firebird_metadata_extract` | Snapshot de DDL completo para versionamento no git |
| `firebird_describe_database` | Visão completa do schema + diagrama ER em Mermaid |
| `firebird_describe_table` | Schema completo de uma tabela (colunas, índices, constraints, triggers) |
| `firebird_list` | Listar tabelas, views, procedures, funções, triggers, índices, sequências |
| `firebird_find_column` | Encontrar coluna por padrão de nome em todas as tabelas |
| `firebird_check_duplicates` | Verificar integridade antes de criar constraint único |
| `firebird_count` | Contagem rápida de linhas (com WHERE opcional) |
| `firebird_sample` | Amostra rápida de dados de uma tabela |
| `firebird_explain_plan` | Inspecionar plano de execução de uma query (sem executá-la) |
| `firebird_generate_dev` | Gerar código (Java, Go, Rust, TS, Protobuf, OpenAPI, Lazarus) |
| `firebird_generate_view_dto` | Gerar DTOs a partir de views |
| `firebird_create_database` | Criar um novo arquivo .fdb |
| `firebird_create_trigger` | Criar trigger (BEFORE/AFTER INSERT/UPDATE/DELETE) |
| `firebird_create_procedure` | Criar procedure armazenada |
| `firebird_create_function` | Criar função definida pelo usuário |
| `firebird_create_sequence` | Criar sequência (generator) |
| `firebird_alter_column_type` | Alterar tipo de coluna usando domain |
| `firebird_drop` | Remover tabela, view, trigger, procedure, função, índice, sequência |
| `firebird_users` | Listar todos os usuários com acesso ao banco |
| `firebird_grants` | Mostrar GRANTs de uma tabela ou de todas |
| `firebird_blob_info` | Listar colunas BLOB com subtipo |
| `firebird_index_usage` | Mostrar todos os índices com colunas e unicidade |
| `firebird_domains` | Analisar domains definidos pelo usuário e uso |
| `firebird_get_databases` | Listar bancos configurados |
| `firebird_crypt_state` | Verificar estado de criptografia em repouso |

---

## Referências

| Documento | Conteúdo |
|---|---|
| [`README.md`](README.md) | Referência completa das tools (todas as 36 com parâmetros e exemplos), em inglês |
| [`README-PTBR.md`](README-PTBR.md) | Idem, em português |
| [`README_COMMAND_LINE.md`](README_COMMAND_LINE.md) | Detalhes do protocolo JSON-RPC, scripts helper |
| [`examples/test-mcp/`](examples/test-mcp/) | Banco de teste completo (schema, dados seed, script de criação) |
| [`examples/mcp-call.sh`](examples/mcp-call.sh) | Helper CLI (Linux/macOS) |
| [`examples/mcp-call.bat`](examples/mcp-call.bat) | Helper CLI (Windows) |

---

## Licença

Ver [LICENSE](LICENSE).
