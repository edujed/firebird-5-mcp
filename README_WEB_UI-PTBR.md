# firebird-5-mcp — Web UI (HTTP + SSE)

> **Use o servidor MCP a partir de um navegador ou de qualquer cliente não-stdio.**
>
> O transporte padrão é **stdio** (JSON-RPC sobre stdin/stdout) — ideal para
> IDEs e ferramentas de CLI que spawnam o binário como subprocesso. Mas web
> UIs, agentes baseados em navegador e clientes que não conseguem spawnar
> processos (llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode, dashboards
> customizados) precisam de um transporte de rede. É para isso que serve o
> **`--sse`**.

Este guia cobre tudo o que você precisa para rodar o `firebird-5-mcp` como um
serviço de rede e conversar com ele a partir de uma web UI:

- Iniciando o servidor em modo SSE
- Os endpoints HTTP (`/sse`, `/messages`, `/files`)
- Autenticação (bearer token)
- CORS (clientes de navegador em outra origem)
- Baixando arquivos gerados a partir do navegador
- Um cliente JavaScript mínimo que você pode colar na sua UI
- Um cliente Python mínimo
- Solução de problemas

---

## Quando usar o modo SSE

| Você quer… | Use |
|---|---|
| Conectar de uma IDE (Zed, VSCode, Cursor, JetBrains) | **stdio** (padrão) |
| Automatizar via shell / cron / CI | **stdio** (padrão) |
| Conectar de um **navegador** (web UI, dashboard, playground) | **SSE** (`--sse`) |
| Conectar de um cliente não-stdio (llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode) | **SSE** (`--sse`) |
| Servir **vários usuários simultâneos** a partir de um servidor | **SSE** (`--sse`) |

> **Nota de segurança:** o modo SSE expõe o conjunto completo de ferramentas
> pela rede — incluindo DDL, DML e geração de código. **Sempre habilite a
> autenticação** (`--token` ou `--token-file`) quando o servidor for
> acessível além do `localhost`. Veja [Autenticação](#autenticação).

---

## Começando rápido

### 1. Configurar

Mesmo `config.json` do modo stdio — veja o [README.md](README.md#configuração)
principal. Mínimo:

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
        "description": "MCP test database",
        "path": "/database/test-mcp.fdb"
      }
    ],
    "default_database": "test-mcp"
  }
}
```

### 2. Iniciar o servidor

```bash
# Desenvolvimento local (sem auth, apenas localhost)
firebird-5-mcp --sse --host 127.0.0.1 --port 9000

# LAN / produção (auth habilitada, todas as interfaces)
firebird-5-mcp --sse --host 0.0.0.0 --port 9000 \
  --token-file /etc/mcp/firebird-token \
  --base-url http://192.168.1.50:9000
```

O servidor imprime `MCP SSE server listening on <host>:<port>` quando está
pronto.

### 3. Abrir o stream SSE

Em um navegador (com auth desabilitada):

```
http://127.0.0.1:9000/sse
```

Você verá um stream de eventos cru. O primeiro evento é `endpoint`, contendo
a URL para onde o cliente deve fazer POST dos requests JSON-RPC:

```
event: endpoint
data: /messages?sessionId=6f1c2a3e-...
```

Esse é o protocolo inteiro — mais detalhes abaixo.

---

## Referência da CLI

```
firebird-5-mcp [OPÇÕES]

TRANSPORTE:
  (sem flags)       stdio — JSON-RPC sobre stdin/stdout (padrão)
  --sse             HTTP+SSE — para web UIs e clientes não-stdio

OPÇÕES SSE:
  --host <addr>       endereço de bind (padrão: 0.0.0.0)
  --port <port>       porta de bind (padrão: 9000)
  --token <token>     bearer token para autenticação
  --token-file <path> caminho para um arquivo contendo o bearer token
  --session-timeout <min>  timeout de inatividade da sessão (padrão: 30)
  --base-url <url>    URL base para os links de download do /files
```

| Flag | Padrão | Descrição |
|------|--------|-----------|
| `--sse` | off | Habilita o transporte HTTP+SSE |
| `--host` | `0.0.0.0` | Endereço de bind. Use `127.0.0.1` para manter local |
| `--port` | `9000` | Porta de bind |
| `--token` | — | Bearer token. Habilita a autenticação |
| `--token-file` | — | Caminho para um arquivo contendo o token (sobrescreve `--token`) |
| `--session-timeout` | `30` | Minutos de inatividade antes de uma sessão ser removida |
| `--base-url` | derivado | URL base anunciada nos campos `download_url` (veja abaixo) |

> **`--base-url` e hosts wildcard:** quando `--host` é um wildcard
> (`0.0.0.0` ou `::`), o servidor não consegue derivar uma URL utilizável
> dele, então os campos `download_url` são omitidos a menos que você passe
> `--base-url` explicitamente. Se você fizer bind em `0.0.0.0` (típico para
> acesso via LAN), passe `--base-url http://<ip-lan>:<porta>` para que as
> ferramentas que geram arquivos anunciem links de download funcionais.

---

## Endpoints HTTP

O servidor implementa o transporte **HTTP+SSE** do MCP (spec `2024-11-05`):

| Endpoint | Método | Propósito |
|----------|--------|-----------|
| `/sse` | `GET` | Abre um stream Server-Sent Events (um por sessão) |
| `/messages?sessionId=<uuid>` | `POST` | Envia um request JSON-RPC para aquela sessão |
| `/files?path=<rel>` | `GET` | Baixa um arquivo do diretório de armazenamento |
| `/files` | `POST` | Envia um arquivo para o diretório de armazenamento (multipart) |
| *(todos)* | `OPTIONS` | Preflight CORS → `204 No Content` |

### `GET /sse` — abrir uma sessão

Abre uma nova sessão SSE. A resposta é um `text/event-stream` com:

1. Evento **`endpoint`** (primeiro) — contém a URL relativa para o POST:
   `/messages?sessionId=<uuid>`
2. Eventos **`message`** — respostas JSON-RPC, uma por request
3. Keep-alive **`ping`** a cada 15 segundos

```
event: endpoint
data: /messages?sessionId=6f1c2a3e-4b2d-4a1c-9e8f-0a1b2c3d4e5f

event: message
data: {"jsonrpc":"2.0","id":1,"result":{...}}

: ping
```

### `POST /messages?sessionId=<uuid>` — enviar um request

Envia um request JSON-RPC 2.0 (método MCP) para a sessão dada.

- **Corpo do request:** um único objeto de request JSON-RPC
  (`{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{...}}`)
- **Resposta:** `202 Accepted` com **corpo vazio** — a resposta real chega
  no stream SSE da sessão como um evento `message`
- **Erros:** `400` (sessionId inválido / JSON inválido), `401` (auth),
  `404` (sessão não encontrada ou expirada)

O `sessionId` é o UUID do evento `endpoint`. Cada `GET /sse` cria uma nova
sessão; você pode abrir quantas precisar.

### `GET /files?path=<rel>` — baixar um arquivo

Streama um arquivo do **diretório de armazenamento** (onde os arquivos
gerados ficam):

- `path` — caminho relativo dentro do diretório de armazenamento, ex.:
  `test-mcp_csv/users.csv` ou `test-mcp_java/users/User.java`
- Resposta: `200` com `Content-Disposition: attachment; filename="..."`
- Erros: `400` (caminho inválido / traversal), `401` (auth), `404` (não
  encontrado)

O diretório de armazenamento é resolvido da configuração ativa:
`config.storage.dir` > `~/.firebird-mcp/output/`.

### `POST /files` — enviar um arquivo

Upload multipart para o diretório de armazenamento:

- Campo **`file`** — o conteúdo do arquivo (binário)
- Campo **`target`** — caminho relativo dentro do diretório de armazenamento
  (os diretórios pai são criados automaticamente)
- Resposta: `201 Created` com `{"path": "...", "size": 12345}`
- **Limite:** 500 MB por arquivo
- Erros: `400` (campos faltando / traversal), `401` (auth), `413` (muito
  grande)

> **Path traversal é bloqueado** nos dois endpoints: componentes `..`,
> caminhos absolutos e escapes via symlink são rejeitados.

---

## Autenticação

A autenticação está **desabilitada por padrão**. Quando habilitada, todo
request para `/sse`, `/messages` e `/files` deve carregar:

```
Authorization: Bearer <token>
```

Habilite com `--token <token>` ou `--token-file <path>` (o arquivo tem
precedência). A comparação é em tempo constante (resistente a timing
attacks).

```bash
# Token inline (ok para desenvolvimento local)
firebird-5-mcp --sse --port 9000 --token s3cret

# Arquivo de token (recomendado para produção)
echo -n "s3cret" > /etc/mcp/firebird-token
chmod 600 /etc/mcp/firebird-token
firebird-5-mcp --sse --host 0.0.0.0 --port 9000 --token-file /etc/mcp/firebird-token
```

Tokens inválidos ou ausentes recebem `401 Unauthorized` com um corpo de erro
JSON-RPC.

> **Escopo:** este é um token estático compartilhado — não implementa
> identidade de usuário, expiração ou rotação. Essas preocupações são
> delegadas a uma interface de autenticação externa (reverse proxy, gateway,
> etc.). A struct `AuthConfig` foi desenhada para que um validador mais
> sofisticado possa ser plugado depois.

---

## CORS (clientes de navegador)

Todos os endpoints SSE suportam CORS para clientes de navegador:

- Requests de preflight `OPTIONS` retornam `204 No Content` com:
  - `Access-Control-Allow-Origin: *`
  - `Access-Control-Allow-Methods: GET, POST, OPTIONS`
  - `Access-Control-Allow-Headers` — ecoa os headers solicitados
  - `Access-Control-Max-Age: 86400`
- Respostas regulares incluem `Access-Control-Allow-Origin: *`

Isso significa que uma web UI hospedada em **qualquer origem** (ex.:
`https://myapp.com`) pode conversar com o servidor MCP (ex.:
`http://192.168.1.50:9000`) diretamente do navegador — sem requisito de
mesma origem, sem proxy.

> **Nota:** o `EventSource` (a API SSE padrão do navegador) **não** suporta
> headers customizados, então com autenticação habilitada você deve usar
> `fetch()` com um leitor SSE manual em vez de `EventSource`. Veja o
> [cliente JavaScript](#cliente-javascript) abaixo.

---

## Sessões

- Cada `GET /sse` cria uma **sessão** (UUID → canal broadcast)
- Sessões são **removidas** após `--session-timeout` minutos de inatividade
  (padrão 30)
- Um `POST /messages` para uma sessão expirada/desconhecida retorna `404`
- Cada chamada de ferramenta roda em sua própria thread (`spawn_blocking`)
  com sua própria conexão Firebird — **sem lock global**, então 20 usuários
  simultâneos = 20 conexões independentes (o MVCC do Firebird lida bem com
  isso)

---

## Download de arquivos a partir do navegador

No modo SSE, toda ferramenta que produz ou referencia um arquivo inclui um
**`download_url`** na resposta — uma URL pronta para uso apontando para o
`GET /files`:

```
{base}/files?path=<caminho relativo url-encoded>
```

Ferramentas afetadas: `firebird_export`, `firebird_import`,
`firebird_describe_file`, `firebird_generate_dev`,
`firebird_generate_view_dto`, `firebird_metadata_extract` (com
`outputDirPath`) e `firebird_list_files`.

A URL também está incluída na resposta `text` legível por humanos
(linhas `Download:` / `Download URLs:`), então agentes que leem apenas o
conteúdo de texto também conseguem recuperar o arquivo.

**Exemplo** — exportar uma tabela e baixá-la do navegador:

```json
// 1. Chamar firebird_export via o protocolo MCP
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "firebird_export",
    "arguments": {
      "source": "USERS",
      "output_path": "/database/output/test-mcp_csv/users.csv",
      "format": "csv",
      "database": "test-mcp"
    }
  }
}

// 2. A resposta inclui (no structuredContent e no texto):
//    download_url: "http://192.168.1.50:9000/files?path=test-mcp_csv%2Fusers.csv"

// 3. Abra essa URL no navegador (ou faça fetch com o header de auth)
```

> **Quando o `download_url` está ausente:** o arquivo deve estar dentro do
> diretório de armazenamento (o endpoint `/files` só serve aquela árvore) e
> a URL base deve ser conhecida (passe `--base-url` quando fizer bind em um
> host wildcard). No modo stdio não há URL base, então apenas o `path`
> absoluto é retornado.

**Listando arquivos:** chame `firebird_list_files` — ele retorna caminhos
relativos que podem ser usados diretamente como o parâmetro de query `path`
para downloads.

---

## Cliente JavaScript

Um cliente mínimo que você pode colar em uma web UI. Ele lida com o
handshake MCP completo (`initialize` → `initialized` → `tools/list` →
`tools/call`) e funciona **com ou sem autenticação** (usa `fetch` + leitura
manual do SSE, então o header `Authorization` funciona — ao contrário do
`EventSource`).

```javascript
/**
 * Cliente SSE mínimo para o firebird-5-mcp.
 *
 * @param {string} baseUrl  ex.: "http://192.168.1.50:9000"
 * @param {string} [token]  bearer token (omita se a auth estiver desabilitada)
 */
class FirebirdMCP {
  constructor(baseUrl, token = null) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.token = token;
    this.endpoint = null;      // preenchido pelo evento SSE 'endpoint'
    this.nextId = 1;
    this.pending = new Map();  // id -> {resolve, reject}
  }

  _headers() {
    const h = { "Content-Type": "application/json" };
    if (this.token) h["Authorization"] = `Bearer ${this.token}`;
    return h;
  }

  /** Abre o stream SSE e faz o handshake MCP. */
  async connect() {
    const res = await fetch(`${this.baseUrl}/sse`, {
      headers: this._headers(),
    });
    if (!res.ok) throw new Error(`Falha ao conectar SSE: ${res.status}`);

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      // Frames SSE são separados por uma linha em branco.
      let sep;
      while ((sep = buffer.indexOf("\n\n")) !== -1) {
        const frame = buffer.slice(0, sep);
        buffer = buffer.slice(sep + 2);
        this._handleFrame(frame);
      }
    }
  }

  _handleFrame(frame) {
    let event = "message";
    const data = [];
    for (const line of frame.split("\n")) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (line.startsWith("data:")) data.push(line.slice(5).trim());
    }
    if (data.length === 0) return;

    if (event === "endpoint") {
      this.endpoint = data[0]; // ex.: "/messages?sessionId=..."
      return;
    }

    if (event === "message") {
      const msg = JSON.parse(data.join("\n"));
      if (msg.id !== undefined && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
      }
    }
  }

  /** Envia um request JSON-RPC e resolve com o resultado. */
  _request(method, params) {
    if (!this.endpoint) throw new Error("Não conectado");
    const id = this.nextId++;
    const body = { jsonrpc: "2.0", id, method, params };
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      fetch(`${this.baseUrl}${this.endpoint}`, {
        method: "POST",
        headers: this._headers(),
        body: JSON.stringify(body),
      }).then((res) => {
        if (!res.ok && res.status !== 202) {
          reject(new Error(`POST falhou: ${res.status}`));
        }
      }).catch(reject);
    });
  }

  /** Lista todas as ferramentas disponíveis. */
  listTools() {
    return this._request("tools/list", {});
  }

  /** Chama uma ferramenta pelo nome. Resolve com o resultado. */
  callTool(name, arguments_) {
    return this._request("tools/call", { name, arguments: arguments_ });
  }
}

// --- Uso -----------------------------------------------------------------

const mcp = new FirebirdMCP("http://192.168.1.50:9000", "s3cret");
await mcp.connect();

// Listar ferramentas
const tools = await mcp.listTools();
console.log(tools.tools.map((t) => t.name));

// Executar uma query
const result = await mcp.callTool("firebird_query", {
  sql: "SELECT * FROM USERS",
  database: "test-mcp",
});
console.log(result.content[0].text);

// Exportar uma tabela e baixar o arquivo
const exportResult = await mcp.callTool("firebird_export", {
  source: "USERS",
  output_path: "/database/output/test-mcp_csv/users.csv",
  format: "csv",
  database: "test-mcp",
});
// A URL de download está no texto (e no structuredContent):
const url = exportResult.structuredContent?.files?.[0]?.download_url;
if (url) {
  const fileRes = await fetch(url, { headers: { Authorization: `Bearer s3cret` } });
  const blob = await fileRes.blob();
  // ... salvar o blob ...
}
```

> **Sem auth**, omita o argumento `token` e o header `Authorization`
> desaparece automaticamente.

---

## Cliente Python

Um cliente mínimo usando `requests` (sem dependências extras além de
`requests`):

```python
import json
import requests

BASE = "http://192.168.1.50:9000"
TOKEN = "s3cret"  # None se a auth estiver desabilitada

headers = {"Authorization": f"Bearer {TOKEN}"} if TOKEN else {}


def connect():
    """Abre o stream SSE, retorna (session_url, response_iterator)."""
    resp = requests.get(f"{BASE}/sse", headers=headers, stream=True)
    resp.raise_for_status()
    # Primeiro evento: endpoint
    for line in resp.iter_lines(decode_unicode=True):
        if line and line.startswith("data:"):
            return line[5:].strip(), resp
    raise RuntimeError("nenhum evento endpoint")


def send(session_url, method, params, req_id):
    """Faz POST de um request JSON-RPC; retorna o status HTTP (202 esperado)."""
    body = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
    r = requests.post(f"{BASE}{session_url}", json=body, headers=headers)
    return r.status_code


def read_response(resp, req_id):
    """Lê linhas SSE até a resposta para req_id chegar."""
    for line in resp.iter_lines(decode_unicode=True):
        if line and line.startswith("data:"):
            msg = json.loads(line[5:].strip())
            if msg.get("id") == req_id:
                return msg
    raise RuntimeError("conexão fechada antes da resposta")


# --- Uso -----------------------------------------------------------------

session_url, resp = connect()

# Handshake
send(session_url, "initialize", {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "python-client", "version": "1.0"},
}, 1)
msg = read_response(resp, 1)
print("servidor:", msg["result"]["serverInfo"])

# Listar ferramentas
send(session_url, "tools/list", {}, 2)
msg = read_response(resp, 2)
print([t["name"] for t in msg["result"]["tools"]])

# Chamar uma ferramenta
send(session_url, "tools/call", {
    "name": "firebird_query",
    "arguments": {"sql": "SELECT * FROM USERS", "database": "test-mcp"},
}, 3)
msg = read_response(resp, 3)
print(msg["result"]["content"][0]["text"])
```

---

## Fluxo do protocolo MCP

O handshake MCP sobre SSE (o que qualquer cliente compliant faz):

```
Cliente                                   Servidor
  │                                         │
  │── GET /sse ────────────────────────────▶│  (header de auth, se habilitada)
  │◀─ event: endpoint ─────────────────────│  data: /messages?sessionId=<uuid>
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":1,"method":"initialize",...}
  │◀─ 202 Accepted (corpo vazio) ──────────│
  │◀─ event: message ──────────────────────│  {"id":1,"result":{serverInfo,...}}
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":2,"method":"notifications/initialized"}
  │◀─ 202 Accepted ────────────────────────│  (sem resposta — é uma notificação)
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":3,"method":"tools/list",...}
  │◀─ 202 Accepted ────────────────────────│
  │◀─ event: message ──────────────────────│  {"id":3,"result":{tools:[...]}}
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":4,"method":"tools/call",...}
  │◀─ 202 Accepted ────────────────────────│
  │◀─ event: message ──────────────────────│  {"id":4,"result":{content:[...]}}
  │                                         │
  │  (pings de keep-alive a cada 15s)       │
```

Pontos-chave:

- **`POST /messages` sempre retorna `202 Accepted` com corpo vazio.**
  A resposta chega no stream SSE, casada pelo campo `id`.
- **Notificações** (sem `id`, ex.: `notifications/initialized`) não
  recebem resposta — o `202` é tudo o que você recebe.
- **Uma sessão por `GET /sse`.** Abra múltiplas sessões se precisar de
  paralelismo; cada uma é independente.
- **Sessões expiram** após `--session-timeout` minutos de inatividade.
  Reconecte com um novo `GET /sse` se receber `404` em um POST.

---

## Rodando o servidor

### systemd (Linux)

```ini
# /etc/systemd/system/firebird-mcp.service
[Unit]
Description=firebird-5-mcp (modo SSE)
After=network.target

[Service]
User=mcp
ExecStart=/usr/local/bin/firebird-5-mcp --sse --host 0.0.0.0 --port 9000 \
    --token-file /etc/mcp/firebird-token \
    --base-url http://192.168.1.50:9000
Restart=on-failure
# A configuração está em /home/mcp/.firebird-mcp/config.json (ou defina FIREBIRD_CONFIG)

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now firebird-mcp
journalctl -u firebird-mcp -f
```

### Docker

```dockerfile
FROM alpine:3.20
COPY firebird-5-mcp /usr/local/bin/firebird-5-mcp
COPY config.json /root/.firebird-mcp/config.json
EXPOSE 9000
ENTRYPOINT ["/usr/local/bin/firebird-5-mcp", "--sse", "--host", "0.0.0.0", "--port", "9000"]
```

> O binário é um único arquivo estático — sem dependências de runtime. O
> `config.json` deve ser montado/copiado para dentro, e os caminhos de banco
> nele devem ser válidos **dentro do container** (monte os arquivos `.fdb`
> como volumes).

### Reverse proxy (nginx)

Se você quiser HTTPS ou um hostname amigável na frente do servidor:

```nginx
server {
    listen 443 ssl;
    server_name mcp.example.com;

    # ssl_certificate ...; ssl_certificate_key ...;

    location / {
        proxy_pass http://127.0.0.1:9000;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        # SSE: desabilitar buffering e timeouts
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 1h;
    }
}
```

> **Importante:** desabilite o `proxy_buffering` e defina um
> `proxy_read_timeout` longo — SSE é um stream de longa duração, e o nginx
> vai bufferizar a resposta ou derrubar a conexão caso contrário.
>
> Ao usar um reverse proxy, passe `--base-url https://mcp.example.com` para
> que os campos `download_url` apontem para a URL pública.

---

## Solução de problemas

| Sintoma | Causa / correção |
|---------|------------------|
| `401 Unauthorized` no `GET /sse` | Auth está habilitada — envie `Authorization: Bearer <token>` em **todo** request (`/sse`, `/messages`, `/files`) |
| `404 Session not found or expired` no POST | A sessão foi removida (inatividade > `--session-timeout`) ou o `sessionId` está errado. Abra um novo `GET /sse` e use o novo `endpoint` |
| Sem `download_url` nas respostas das ferramentas | O arquivo está fora do diretório de armazenamento, **ou** a URL base é desconhecida (`--host` wildcard sem `--base-url`). Passe `--base-url http://<host>:<porta>` |
| Navegador não conecta (erro CORS) | Verifique se o servidor está rodando e se a porta é acessível. Todos os endpoints enviam `Access-Control-Allow-Origin: *` — se você vir um erro CORS, o request provavelmente nunca chegou ao servidor (firewall / host errado) |
| `EventSource` não consegue enviar o header de auth | `EventSource` não suporta headers customizados. Use `fetch()` com um leitor SSE manual (veja o [cliente JavaScript](#cliente-javascript)) |
| Conexão cai após ~60s atrás de um proxy | O proxy está bufferizando ou dando timeout no stream SSE. Desabilite o buffering e aumente o `proxy_read_timeout` (veja [nginx](#reverse-proxy-nginx)) |
| `413 Payload Too Large` no upload | O arquivo excede o limite de 500 MB |
| `400 path traversal not allowed` | O `path`/`target` contém `..`, é absoluto ou escapa o diretório de armazenamento |
| Servidor faz bind mas nada escuta na porta esperada | Verifique o `--port` (padrão `9000`) e que nenhum outro processo a ocupe |

---

## Referências

| Documento | Conteúdo |
|---|---|
| [`README.md`](README.md) | Referência completa das tools (todas com parâmetros e exemplos), em inglês |
| [`README-PTBR.md`](README-PTBR.md) | Idem, em português |
| [`README_COMMAND_LINE.md`](README_COMMAND_LINE.md) | Transporte stdio, protocolo JSON-RPC, scripts helper |
| [`README_MIGRATIONS.md`](README_MIGRATIONS.md) | Migração de dados (Parquet/IPC/CSV/DBF/JSONL) |
| [`README_CODEGEN.md`](README_CODEGEN.md) | Exemplos de geração de código (todas as linguagens) |
| [`README_FOR_HUMANS.md`](README_FOR_HUMANS.md) | Uso como CLI puro — sem agente de IA |
| [Spec MCP `2024-11-05`](https://modelcontextprotocol.io/specification/2024-11-05) | A especificação do transporte HTTP+SSE |

---

## Licença

Ver [LICENSE](LICENSE).
