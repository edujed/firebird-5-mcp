# firebird-5-mcp: o Firebird na era dos agentes de IA — do schema ao código, do banco local ao data lake, nuvem e BI

*Por Eduardo Jedliczka (e Qwen 3.8), para a Comunidade Firebird*

---

Se você trabalha com Firebird há algum tempo, conhece a sensação: o banco é sólido, previsível, roda em qualquer lugar — mas, quando o assunto é **agentes de IA**, ele simplesmente não existe. O ecossistema MCP (Model Context Protocol) é dominado por Postgres e MySQL; para Firebird, a resposta costuma ser "use isql" — e isql não conversa com LLMs.

O **firebird-5-mcp** nasceu para quebrar esse impasse. É um servidor MCP escrito em Rust puro, distribuído como **um único binário estático**: sem `isql`, sem ODBC, sem DLLs, sem `.so`. Uma linha de comando, e o Firebird passa a ser lido, interrogado, migrado e até convertido em código-fonte por um agente de IA — ou por um script de cron, que é o bônus que ninguém te contou.

Neste artigo, vou apresentar a visão por trás do projeto, o que ele cobre (e o que ele se recusa a fazer), e — principalmente — **como usá-lo de forma eficiente**, com exemplos reais.

## O problema: um banco invisível para a IA

O Model Context Protocol é o padrão que conecta modelos de linguagem a ferramentas externas. É assim que um agente de IA lê arquivos, consulta bancos, chama APIs. E o padrão funciona: você registra o servidor no seu editor (VSCode, Cursor, Zed, JetBrains) e o agente ganha acesso a um conjunto de *tools* com nomes, parâmetros e descrições.

O Firebird ficou de fora dessa onda por uma razão prática: **não havia um driver limpo**. As opções clássicas exigiam bibliotecas nativas, ODBC ou o shell-out para `isql` — tudo frágil em containers, pipelines de CI e ambientes sem GUI. Para um projeto que quer rodar em qualquer lugar, isso era um dealbreaker.

A pergunta que motiva o firebird-5-mcp é simples: **como dar a um agente de IA — ou a um script — acesso completo e previsível ao Firebird, sem nenhuma dependência externa?**

## Visão e objetivo

A resposta do projeto é direta: um servidor MCP **puro-Rust**, baseado no driver `rsfbclient`, que expõe o banco como um conjunto de ferramentas JSON-RPC. O objetivo tem duas faces:

1. **Ser a ponte entre o Firebird e o mundo LLM.** O agente descreve o schema, consulta dados, valida integridade, gera código e extrai DDL — tudo sem tocar em um shell.
2. **Ser, ao mesmo tempo, um CLI completo.** O mesmo binário que alimenta o agente de IA roda em cron jobs, pipelines de CI e scripts de shell, sem LLM nenhum. Pense nele como um *Firebird PowerToys*: uma faca suíça para o trabalho diário com banco de dados.

Há um detalhe técnico que merece destaque: o projeto usa uma **cópia vendida do `rsfbclient` com um patch de precisão NUMERIC/DECIMAL**. O driver upstream converte inteiros escalados para `f64`, o que perde exatidão — e `NUMERIC(18,4)` é o tipo padrão para dinheiro em schemas Firebird. O patch mantém o inteiro escalado e o scale, renderizando o decimal exato como texto no fetch. Para quem lida com valores monetários, isso não é detalhe: é correção.

## Escopo: o que o projeto cobre — e o que ele se recusa a fazer

O firebird-5-mcp expõe **mais de 36 ferramentas**, organizadas em nove famílias:

| Família | Ferramentas | Para quê |
|---|---|---|
| **Schema & Estrutura** | `describe_database`, `describe_table`, `describe`, `list`, `check_duplicates` | Visão completa do banco, com ER diagram em Mermaid |
| **DDL & Avançado** | `create_database`, `create_trigger`, `create_procedure`, `create_function`, `create_sequence`, `alter_column_type`, `drop`, `run_script`, `execute_immediate` | Criar e modificar objetos sem isql |
| **Query & Dados** | `query`, `count`, `sample`, `insert_batch` | SQL livre, contagens, amostras, carga em lote |
| **Code Generation** | `generate_dev`, `generate_view_dto` | Código em 11 linguagens a partir do schema |
| **Discovery** | `get_databases`, `metadata_extract`, `explain_plan`, `find_column` | Substitutos de `isql -x`, planos de execução, busca de colunas |
| **Introspecção** | `table_stats`, `users`, `grants`, `blob_info`, `index_usage`, `domains`, `crypt_state`, `database_stats`, `query_history` | Monitoramento estilo gstat + MON$ |
| **Qualidade de Dados** | `data_quality` | Regras: NULLs, strings vazias, outliers (IQR), datas fora de faixa |
| **Arquivos** | `list_files` | Listar artefatos gerados (codegen, export, metadata) |
| **Migração** | `export`, `import`, `describe_file` | Parquet, Arrow IPC, CSV, DBF, JSON Lines — export e import |

Dois pontos de escopo merecem atenção:

**O que está dentro:** introspeção de schema, DDL, DML, geração de código, migração de dados bidirecional, qualidade de dados, monitoramento via tabelas `MON$` e um sistema de eventos universais (toda tool aceita `on_success_event`/`on_fail_event`, que dispara `POST_EVENT` no Firebird e registra em `events.jsonl`).

**O que está fora, por design:** o projeto é explícito — **não é recomendado para bancos de produção**. Ferramentas de codegen, DDL e refatoração podem ser destrutivas. Para produção, use uma ferramenta de administração dedicada (`isql`, `fbguard`, ou a suíte do seu DBA). Há um modo *safe-readonly* a caminho, que resolverá essa limitação.

**Transportes:** o binário fala **stdio** (JSON-RPC sobre stdin/stdout — o padrão para IDEs e CLI) e **HTTP+SSE** (spec `2024-11-05` — para web UIs, clientes não-stdio como llama.cpp `--ui-mcp-proxy`, CODEX e ClaudeCode, e múltiplos usuários simultâneos).

## Vantagens: por que isso importa

| Vantagem | Por quê importa |
|---|---|
| **Zero dependências nativas** | Binário estático: roda em container, CI, Windows, macOS, Linux — sem isql, ODBC ou DLL |
| **Do schema ao código** | `describe_database` gera ER em Mermaid; `generate_dev` produz entidades Java (JPA+Lombok), structs Go, structs Rust, interfaces TypeScript, Protobuf, OpenAPI, GraphQL, Python (SQLAlchemy+Pydantic), JSON Schema, formulários Lazarus e páginas Svelte 5 |
| **Precisão NUMERIC/DECIMAL** | Patch no driver: dinheiro exato, sem arredondamento de `f64` |
| **Dupla natureza: MCP + CLI** | O mesmo binário alimenta agentes de IA **e** cron jobs/ETL |
| **Segurança embutida** | `allowed_paths` restringe onde o banco pode ser criado; auth bearer-token no modo SSE; `explain_plan` roda em transação rollback |
| **Observabilidade** | `database_stats` (snapshot MON$DATABASE), `query_history` (quem está rodando o quê), `crypt_state` (criptografia em repouso), `data_quality` |
| **Eventos universais** | `on_success_event`/`on_fail_event` → `POST_EVENT` + log JSONL, sem nunca falhar a chamada |

A combinação mais interessante é a **dupla natureza**. Um projeto típico usa o firebird-5-mcp de duas formas ao mesmo tempo: o agente de IA no editor para desenvolvimento interativo, e o mesmo binário em um cron noturno para exportar tabelas para o DataLake, e Relatórios via PowerBI. Uma configuração, dois mundos.

## Como utilizar de forma eficiente

### 1. Setup em três passos

**Passo 1 — Obter o binário.** Baixe o `.tgz` ou `.zip` da sua plataforma nos [Releases](https://github.com/edujed/firebird-5-mcp/releases) e extraia, ou compile do source:

```bash
git clone https://github.com/edujed/firebird-5-mcp.git
cd firebird-5-mcp
./scripts/build.sh          # compila → ~/.local/bin/firebird-5-mcp
```

**Passo 2 — Configurar.** Crie `~/.firebird-mcp/config.json` (o binário cria um vazio no primeiro run):

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
        "description": "Banco de teste do MCP",
        "path": "/database/test-mcp.fdb"
      }
    ],
    "default_database": "test-mcp"
  }
}
```

**Passo 3 — Registrar na IDE.** No VSCode, abra a Command Palette (`Ctrl+Shift+P`), execute `MCP: Add Server`, escolha `Command (stdio)` e insira o caminho do binário. Ou, de forma permanente, crie `.copilot/mcp-config.json` na raiz do projeto:

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

Pronto. O agente agora enxerga todas as ferramentas do Firebird.

### 2. Uma conversa real com o agente

Para você ver como isso funciona na prática, aqui está uma **transcrição real** de uma sessão com o MCP registrado no editor — sem edição, com os dados reais do banco de teste `test-mcp` (12 tabelas, 14 relacionamentos, ~9.4 MB):

> **Você:** Quais tools de Firebird você tem acesso?
>
> **Agente:** Tenho acesso a 36+ ferramentas organizadas em nove famílias: *Schema & Estrutura* (`describe_database`, `describe_table`, `describe`, `list`, `check_duplicates`), *DDL & Avançado* (`create_database`, `create_trigger`, `create_procedure`, `create_function`, `create_sequence`, `alter_column_type`, `drop`, `run_script`, `execute_immediate`), *Query & Dados* (`query`, `count`, `sample`, `insert_batch`), *Code Generation* (`generate_dev`, `generate_view_dto`), *Discovery* (`get_databases`, `metadata_extract`, `explain_plan`, `find_column`), *Introspecção* (`table_stats`, `users`, `grants`, `blob_info`, `index_usage`, `domains`, `crypt_state`, `database_stats`, `query_history`), *Qualidade de Dados* (`data_quality`), *Arquivos* (`list_files`) e *Migração* (`export`, `import`, `describe_file`).
>
> **Você:** Faz o describe do banco `test-mcp`.
>
> **Agente:** *(chama `firebird_describe_database` com `include_counts=true`)*
>
> **Banco: test-mcp**
> - **Saúde:** ODS 13.1, page size 32768, ~9.4 MB, dialect 3, online, forced writes, 1 conexão ativa
> - **Tabelas (12):** `ACTION_LOG` (1500 linhas), `APPROVAL` (462), `BRANCH` (4), `DEPARTMENT` (35), `PERMISSION` (10), `PROFILE` (5), `PROFILE_PERMISSION` (25), `PROFILE_SCREEN` (20), `SCREEN` (8), `USERS` (100), `USER_DEPARTMENT` (373), `USER_PROFILE` (100)
> - **Relacionamentos (14):** `USERS → ACTION_LOG`, `USERS → APPROVAL` (requester e approver), `BRANCH → DEPARTMENT`, `DEPARTMENT → DEPARTMENT` (self-ref), `PERMISSION → PROFILE_PERMISSION`, `PROFILE → PROFILE_PERMISSION`, `PROFILE → PROFILE_SCREEN`, `SCREEN → PROFILE_SCREEN`, `DEPARTMENT → USERS`, `DEPARTMENT → USER_DEPARTMENT`, `USERS → USER_DEPARTMENT`, `PROFILE → USER_PROFILE`, `USERS → USER_PROFILE`
>
> *(O resultado inclui também o diagrama ER em Mermaid, pronto para renderizar.)*
>
> **Você:** Quantos usuários ativos existem na tabela USERS?
>
> **Agente:** *(chama `firebird_query`)*
>
> ```sql
> SELECT COUNT(*) FROM USERS WHERE ACTIVE = TRUE
> ```
> **Resultado:** 87 usuários ativos.

Note o padrão: o agente **escolhe a ferramenta certa para cada pergunta**. Para "quantos usuários ativos", ele não precisa de `describe` — uma `query` direta resolve. Para "me mostra o schema", ele usa `describe_database` em vez de dezenas de `SELECT` nas tabelas `RDB$`. É isso que torna o uso eficiente: **as ferramentas são o vocabulário, e o agente sabe qual palavra usar.**

### 3. Padrões de uso com agentes de IA

O fluxo recomendado para um agente trabalhando em um projeto Firebird:

1. **`describe_database`** — visão geral + ER diagram. O ponto de partida para qualquer tarefa.
2. **`find_column` / `describe_table`** — localize colunas e tabelas específicas sem varrer o schema inteiro.
3. **`query` / `explain_plan`** — consulte dados e inspecione planos de execução antes de rodar queries caras.
4. **`generate_dev`** — gere o código a partir do schema.

Dicas de eficiência:

- **Use `default_database`.** Se a maioria das suas chamadas vai para o mesmo banco, defina `default_database` no config e omita o parâmetro `database` nas ferramentas. Menos tokens, menos ruído.
- **Prefira ferramentas específicas a queries genéricas.** `firebird_count` é mais barato que `SELECT COUNT(*)` em termos de contexto; `firebird_sample` é mais barato que `SELECT *` quando você só quer ver a forma dos dados.
- **`explain_plan` antes de queries destrutivas.** A ferramenta executa a statement em uma transação que é sempre rollback — você vê o plano sem efeito colateral.
- **`check_duplicates` antes de criar índices únicos.** A ferramenta retorna as linhas duplicadas completas, não apenas o count — o agente pode propor a correção.

### 4. Uso sem IA: o CLI

Se você não usa agente de IA (ou quer automatizar), o mesmo binário é um CLI completo. O helper `examples/mcp-call.sh` encapsula o protocolo JSON-RPC:

```bash
# Listar tabelas
./examples/mcp-call.sh firebird_list '{"type":"TABLES","database":"test-mcp"}'

# Exportar para Parquet (DataLake)
./examples/mcp-call.sh firebird_export '{
  "source": "USERS",
  "database": "test-mcp",
  "output_path": "/datalake/firebird/users.parquet",
  "format": "parquet",
  "compression": "snappy"
}'

# Health check com alerta
OUTPUT=$(./examples/mcp-call.sh firebird_database_stats '{"database":"test-mcp"}')
echo "$OUTPUT" | grep -q "Read-Only: YES" && \
  echo "ALERT: banco em modo somente leitura!" | mail -s "Firebird" dba@exemplo.com

# Snapshot de schema para versionamento
./examples/mcp-call.sh firebird_metadata_extract '{"database":"test-mcp"}' > schema.sql
git diff schema.sql   # rastreie cada mudança
```

Receitas prontas para cron: export noturno para o DataLake, fechamento mensal via `EXECUTE PROCEDURE`, diff de schema com alerta por e-mail, health check diário! Ou seja, além de tudo é CI/CD friendly!

### 5. Escolha o transporte certo

| Cenário | Transporte |
|---|---|
| IDE (VSCode, Cursor, Zed, JetBrains) | **stdio** (padrão) |
| Shell / cron / CI | **stdio** (padrão) |
| Browser (web UI, dashboard) | **SSE** (`--sse`) |
| Cliente não-stdio (llama.cpp, CODEX, ClaudeCode) | **SSE** (`--sse`) |
| Múltiplos usuários simultâneos | **SSE** (`--sse`) |

No modo SSE, **sempre habilite autenticação** quando o servidor estiver acessível fora de `localhost`:

```bash
firebird-5-mcp --sse --host 0.0.0.0 --port 9000 \
  --token-file /etc/mcp/firebird-token \
  --base-url http://192.168.1.50:9000
```

O `--base-url` é importante quando você binda em `0.0.0.0`: sem ele, os links de download de arquivos gerados não funcionam.

### 6. Boas práticas e limites

- **Não use em produção (ainda).** O projeto é para desenvolvimento e análise. O modo *safe-readonly* está a caminho.
- **`COUNT(*)` em tabelas grandes é lento.** `firebird_table_stats` em todos os bancos pode demorar — use com moderação.
- **SSE expõe o toolset completo pela rede** — incluindo DDL e DML. Auth não é opcional fora de localhost.
- **`allowed_paths` é sua âncora de segurança.** Restrinja os diretórios onde o MCP pode criar bancos e escrever arquivos.

## Cenários práticos

**Cenário 1 — O agente gera código.** Você pede ao agente: "gere as entidades Java para as tabelas `USERS` e `DEPARTMENT`". Ele chama `firebird_generate_dev` com `language: "java"` e `include_fks: true`, e recebe classes JPA com Lombok, `@ManyToOne` para as FKs e `@Column` com os tamanhos corretos. O código cai no diretório de saída, pronto para compilar.

**Cenário 2 — Export noturno para o DataLake.** Um cron job chama `firebird_export` para cada tabela relevante, em Parquet com Snappy. O DataLake recebe os arquivos às 2h da manhã, e o BI consome. Sem ODBC, sem ETL tool, sem daemon.

**Cenário 3 — Health check com alerta.** Um script diário chama `firebird_database_stats`, verifica se o banco está em modo read-only ou se há conexões estagnadas, e dispara um alerta. O DBA é notificado antes do usuário.

## Conclusão

O firebird-5-mcp não inventa nada novo — ele conecta o Firebird a um padrão que já existe. O que ele muda é a **acessibilidade**: o Firebird deixa de ser invisível para agentes de IA, e passa a ser tão manipulável quanto um arquivo no disco. O mesmo binário que alimenta o agente no editor exporta tabelas para o DataLake em um cron noturno, gera código em 11 linguagens, valida qualidade de dados e versiona o schema no git.

Para a comunidade Firebird, isso é mais do que uma ferramenta: é um argumento. Se o seu banco pode ser lido por um agente de IA, exportado para Parquet e convertido em código-fonte com um único binário estático, ele está pronto para a próxima década — não apenas para o legado.

**O que vem:** modo *safe-readonly*, para que o projeto possa ser usado em produção com segurança.

**Links:**
- [Repositório](https://github.com/edujed/firebird-5-mcp) — código, docs, exemplos
- [Releases](https://github.com/edujed/firebird-5-mcp/releases) — binários para todas as plataformas
- [README completo](https://github.com/edujed/firebird-5-mcp/blob/main/README.md) — referência de todas as 36+ ferramentas
- [README em PT-BR](https://github.com/edujed/firebird-5-mcp/blob/main/README-PTBR.md)
- [Guia de migrações](https://github.com/edujed/firebird-5-mcp/blob/main/README_MIGRATIONS.md)
- [Guia de codegen](https://github.com/edujed/firebird-5-mcp/blob/main/README_CODEGEN.md)
- [Guia Web UI (SSE)](https://github.com/edujed/firebird-5-mcp/blob/main/README_WEB_UI.md)

*O firebird-5-mcp é licenciado sob MIT. O driver `rsfbclient` é de [crates.io](https://crates.io/crates/rsfbclient); a biblioteca Svelte 5 usada no codegen é [jedSvelted-ui](https://github.com/edujed/jedSvelted-ui).*
