# firebird-5-mcp — Web UI (HTTP + SSE)

> **Use the MCP server from a browser or any non-stdio client.**
>
> The default transport is **stdio** (JSON-RPC over stdin/stdout) — right for
> IDEs and CLI tools that spawn the binary as a subprocess. But web UIs,
> browser-based agents, and clients that can't spawn processes (llama.cpp
> `--ui-mcp-proxy`, CODEX, ClaudeCode, custom dashboards) need a network
> transport. That's what **`--sse`** is for.

This guide covers everything you need to run `firebird-5-mcp` as a network
service and talk to it from a web UI:

- Starting the server in SSE mode
- The HTTP endpoints (`/sse`, `/messages`, `/files`)
- Authentication (bearer token)
- CORS (browser clients on a different origin)
- Downloading generated files from the browser
- A minimal JavaScript client you can paste into your UI
- A minimal Python client
- Troubleshooting

---

## When to use SSE mode

| You want to… | Use |
|---|---|
| Connect from an IDE (Zed, VSCode, Cursor, JetBrains) | **stdio** (default) |
| Script from the shell / cron / CI | **stdio** (default) |
| Connect from a **browser** (web UI, dashboard, playground) | **SSE** (`--sse`) |
| Connect from a non-stdio client (llama.cpp `--ui-mcp-proxy`, CODEX, ClaudeCode) | **SSE** (`--sse`) |
| Serve **multiple concurrent users** from one server | **SSE** (`--sse`) |

> **Security note:** SSE mode exposes the full tool set over the network —
> including DDL, DML, and code generation. **Always enable authentication**
> (`--token` or `--token-file`) when the server is reachable beyond
> `localhost`. See [Authentication](#authentication).

---

## Quick start

### 1. Configure

Same `config.json` as stdio mode — see the main [README.md](README.md#configuration).
Minimum:

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

### 2. Start the server

```bash
# Local development (no auth, localhost only)
firebird-5-mcp --sse --host 127.0.0.1 --port 9000

# LAN / production (auth enabled, all interfaces)
firebird-5-mcp --sse --host 0.0.0.0 --port 9000 \
  --token-file /etc/mcp/firebird-token \
  --base-url http://192.168.1.50:9000
```

The server prints `MCP SSE server listening on <host>:<port>` when ready.

### 3. Open the SSE stream

In a browser (with auth disabled):

```
http://127.0.0.1:9000/sse
```

You'll see a raw event stream. The first event is `endpoint`, carrying the
URL your client should POST JSON-RPC requests to:

```
event: endpoint
data: /messages?sessionId=6f1c2a3e-...
```

That's the whole protocol — more below.

---

## CLI reference

```
firebird-5-mcp [OPTIONS]

TRANSPORT:
  (no flags)        stdio — JSON-RPC over stdin/stdout (default)
  --sse             HTTP+SSE — for web UIs and non-stdio clients

SSE OPTIONS:
  --host <addr>       bind address (default: 0.0.0.0)
  --port <port>       bind port (default: 9000)
  --token <token>     bearer token for authentication
  --token-file <path> path to a file containing the bearer token
  --session-timeout <min>  session inactivity timeout (default: 30)
  --base-url <url>    base URL for /files download links
```

| Flag | Default | Description |
|------|---------|-------------|
| `--sse` | off | Enable the HTTP+SSE transport |
| `--host` | `0.0.0.0` | Bind address. Use `127.0.0.1` to keep it local |
| `--port` | `9000` | Bind port |
| `--token` | — | Bearer token. Enables authentication |
| `--token-file` | — | Path to a file containing the token (overrides `--token`) |
| `--session-timeout` | `30` | Minutes of inactivity before a session is reaped |
| `--base-url` | derived | Base URL advertised in `download_url` fields (see below) |

> **`--base-url` and wildcard hosts:** when `--host` is a wildcard
> (`0.0.0.0` or `::`), the server can't derive a usable URL from it, so
> `download_url` fields are omitted unless you pass `--base-url` explicitly.
> If you bind to `0.0.0.0` (typical for LAN access), pass
> `--base-url http://<lan-ip>:<port>` so file-generating tools can advertise
> working download links.

---

## HTTP endpoints

The server implements the MCP **HTTP+SSE** transport (spec `2024-11-05`):

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/sse` | `GET` | Opens a Server-Sent Events stream (one per session) |
| `/messages?sessionId=<uuid>` | `POST` | Sends a JSON-RPC request for that session |
| `/files?path=<rel>` | `GET` | Downloads a file from the storage directory |
| `/files` | `POST` | Uploads a file to the storage directory (multipart) |
| *(all)* | `OPTIONS` | CORS preflight → `204 No Content` |

### `GET /sse` — open a session

Opens a new SSE session. The response is an `text/event-stream` with:

1. **`endpoint`** event (first) — carries the relative URL to POST to:
   `/messages?sessionId=<uuid>`
2. **`message`** events — JSON-RPC responses, one per request
3. **`ping`** keep-alive every 15 seconds

```
event: endpoint
data: /messages?sessionId=6f1c2a3e-4b2d-4a1c-9e8f-0a1b2c3d4e5f

event: message
data: {"jsonrpc":"2.0","id":1,"result":{...}}

: ping
```

### `POST /messages?sessionId=<uuid>` — send a request

Sends a JSON-RPC 2.0 request (MCP method) for the given session.

- **Request body:** a single JSON-RPC request object
  (`{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{...}}`)
- **Response:** `202 Accepted` with an **empty body** — the actual response
  arrives on the session's SSE stream as a `message` event
- **Errors:** `400` (bad sessionId / bad JSON), `401` (auth), `404`
  (session not found or expired)

The `sessionId` is the UUID from the `endpoint` event. Each `GET /sse`
creates a new session; you can open as many as you need.

### `GET /files?path=<rel>` — download a file

Streams a file from the **storage directory** (where generated files land):

- `path` — relative path within the storage directory, e.g.
  `test-mcp_csv/users.csv` or `test-mcp_java/users/User.java`
- Response: `200` with `Content-Disposition: attachment; filename="..."`
- Errors: `400` (bad path / traversal), `401` (auth), `404` (not found)

The storage directory is resolved from the live config:
`config.storage.dir` > `~/.firebird-mcp/output/`.

### `POST /files` — upload a file

Multipart upload to the storage directory:

- **`file`** field — the file content (binary)
- **`target`** field — relative path within the storage directory
  (parent directories are created automatically)
- Response: `201 Created` with `{"path": "...", "size": 12345}`
- **Limit:** 500 MB per file
- Errors: `400` (missing fields / traversal), `401` (auth), `413` (too large)

> **Path traversal is blocked** on both endpoints: `..` components, absolute
> paths, and symlink escapes are rejected.

---

## Authentication

Authentication is **disabled by default**. When enabled, every request to
`/sse`, `/messages`, and `/files` must carry:

```
Authorization: Bearer <token>
```

Enable it with `--token <token>` or `--token-file <path>` (file takes
precedence). The comparison is constant-time (timing-attack resistant).

```bash
# Inline token (fine for local development)
firebird-5-mcp --sse --port 9000 --token s3cret

# Token file (recommended for production)
echo -n "s3cret" > /etc/mcp/firebird-token
chmod 600 /etc/mcp/firebird-token
firebird-5-mcp --sse --host 0.0.0.0 --port 9000 --token-file /etc/mcp/firebird-token
```

Invalid or missing tokens get `401 Unauthorized` with a JSON-RPC error body.

> **Scope:** this is a static shared token — it doesn't implement user
> identity, expiration, or rotation. Those concerns are delegated to an
> external auth interface (reverse proxy, gateway, etc.). The `AuthConfig`
> struct is designed so a more sophisticated validator can be plugged in
> later.

---

## CORS (browser clients)

All SSE endpoints support CORS for browser clients:

- Preflight `OPTIONS` requests return `204 No Content` with:
  - `Access-Control-Allow-Origin: *`
  - `Access-Control-Allow-Methods: GET, POST, OPTIONS`
  - `Access-Control-Allow-Headers` — echoes the requested headers
  - `Access-Control-Max-Age: 86400`
- Regular responses include `Access-Control-Allow-Origin: *`

This means a web UI hosted on **any origin** (e.g. `https://myapp.com`) can
talk to the MCP server (e.g. `http://192.168.1.50:9000`) directly from the
browser — no same-origin requirement, no proxy needed.

> **Note:** `EventSource` (the standard browser SSE API) does **not** support
> custom headers, so with authentication enabled you must use `fetch()` with
> a manual SSE reader instead of `EventSource`. See the
> [JavaScript client](#javascript-client) below.

---

## Sessions

- Each `GET /sse` creates a **session** (UUID → broadcast channel)
- Sessions are **reaped** after `--session-timeout` minutes of inactivity
  (default 30)
- A `POST /messages` to an expired/unknown session returns `404`
- Each tool call runs in its own thread (`spawn_blocking`) with its own
  Firebird connection — **no global lock**, so 20 concurrent users = 20
  independent connections (Firebird MVCC handles this well)

---

## File downloads from the browser

In SSE mode, every tool that produces or references a file includes a
**`download_url`** in its response — a ready-to-use URL pointing at
`GET /files`:

```
{base}/files?path=<url-encoded relative path>
```

Affected tools: `firebird_export`, `firebird_import`,
`firebird_describe_file`, `firebird_generate_dev`,
`firebird_generate_view_dto`, `firebird_metadata_extract` (with
`outputDirPath`), and `firebird_list_files`.

The URL is also included in the human-readable `text` response
(`Download:` / `Download URLs:` lines), so agents that only read the text
content can retrieve the file too.

**Example** — export a table, then download it from the browser:

```json
// 1. Call firebird_export via the MCP protocol
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

// 2. Response includes (in structuredContent and in the text):
//    download_url: "http://192.168.1.50:9000/files?path=test-mcp_csv%2Fusers.csv"

// 3. Open that URL in the browser (or fetch it with the auth header)
```

> **When `download_url` is missing:** the file must be inside the storage
> directory (the `/files` endpoint only serves that tree), and the base URL
> must be known (pass `--base-url` when binding to a wildcard host). In
> stdio mode there is no base URL, so only the absolute `path` is returned.

**Listing files:** call `firebird_list_files` — it returns relative paths
that can be used directly as the `path` query parameter for downloads.

---

## JavaScript client

A minimal client you can paste into a web UI. It handles the full MCP
handshake (`initialize` → `initialized` → `tools/list` → `tools/call`) and
works **with or without authentication** (uses `fetch` + manual SSE reading,
so the `Authorization` header works — unlike `EventSource`).

```javascript
/**
 * Minimal firebird-5-mcp SSE client.
 *
 * @param {string} baseUrl  e.g. "http://192.168.1.50:9000"
 * @param {string} [token]  bearer token (omit if auth is disabled)
 */
class FirebirdMCP {
  constructor(baseUrl, token = null) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.token = token;
    this.endpoint = null;      // filled by the 'endpoint' SSE event
    this.nextId = 1;
    this.pending = new Map();  // id -> {resolve, reject}
  }

  _headers() {
    const h = { "Content-Type": "application/json" };
    if (this.token) h["Authorization"] = `Bearer ${this.token}`;
    return h;
  }

  /** Opens the SSE stream and performs the MCP handshake. */
  async connect() {
    const res = await fetch(`${this.baseUrl}/sse`, {
      headers: this._headers(),
    });
    if (!res.ok) throw new Error(`SSE connect failed: ${res.status}`);

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      // SSE frames are separated by a blank line.
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
      this.endpoint = data[0]; // e.g. "/messages?sessionId=..."
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

  /** Sends a JSON-RPC request and resolves with the result. */
  _request(method, params) {
    if (!this.endpoint) throw new Error("Not connected");
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
          reject(new Error(`POST failed: ${res.status}`));
        }
      }).catch(reject);
    });
  }

  /** Lists all available tools. */
  listTools() {
    return this._request("tools/list", {});
  }

  /** Calls a tool by name. Resolves with the tool result. */
  callTool(name, arguments_) {
    return this._request("tools/call", { name, arguments: arguments_ });
  }
}

// --- Usage ---------------------------------------------------------------

const mcp = new FirebirdMCP("http://192.168.1.50:9000", "s3cret");
await mcp.connect();

// List tools
const tools = await mcp.listTools();
console.log(tools.tools.map((t) => t.name));

// Run a query
const result = await mcp.callTool("firebird_query", {
  sql: "SELECT * FROM USERS",
  database: "test-mcp",
});
console.log(result.content[0].text);

// Export a table, then download the file
const exportResult = await mcp.callTool("firebird_export", {
  source: "USERS",
  output_path: "/database/output/test-mcp_csv/users.csv",
  format: "csv",
  database: "test-mcp",
});
// The download URL is in the text (and in structuredContent):
const url = exportResult.structuredContent?.files?.[0]?.download_url;
if (url) {
  const fileRes = await fetch(url, { headers: { Authorization: `Bearer s3cret` } });
  const blob = await fileRes.blob();
  // ... save the blob ...
}
```

> **Without auth**, drop the `token` argument and the `Authorization` header
> disappears automatically.

---

## Python client

A minimal client using `requests` (no extra dependencies beyond `requests`):

```python
import json
import requests

BASE = "http://192.168.1.50:9000"
TOKEN = "s3cret"  # None if auth is disabled

headers = {"Authorization": f"Bearer {TOKEN}"} if TOKEN else {}


def connect():
    """Open the SSE stream, return (session_url, response_iterator)."""
    resp = requests.get(f"{BASE}/sse", headers=headers, stream=True)
    resp.raise_for_status()
    # First event: endpoint
    for line in resp.iter_lines(decode_unicode=True):
        if line and line.startswith("data:"):
            return line[5:].strip(), resp
    raise RuntimeError("no endpoint event")


def send(session_url, method, params, req_id):
    """POST a JSON-RPC request; returns the HTTP status (202 expected)."""
    body = {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
    r = requests.post(f"{BASE}{session_url}", json=body, headers=headers)
    return r.status_code


def read_response(resp, req_id):
    """Read SSE lines until the response for req_id arrives."""
    for line in resp.iter_lines(decode_unicode=True):
        if line and line.startswith("data:"):
            msg = json.loads(line[5:].strip())
            if msg.get("id") == req_id:
                return msg
    raise RuntimeError("connection closed before response")


# --- Usage ---------------------------------------------------------------

session_url, resp = connect()

# Handshake
send(session_url, "initialize", {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "python-client", "version": "1.0"},
}, 1)
msg = read_response(resp, 1)
print("server:", msg["result"]["serverInfo"])

# List tools
send(session_url, "tools/list", {}, 2)
msg = read_response(resp, 2)
print([t["name"] for t in msg["result"]["tools"]])

# Call a tool
send(session_url, "tools/call", {
    "name": "firebird_query",
    "arguments": {"sql": "SELECT * FROM USERS", "database": "test-mcp"},
}, 3)
msg = read_response(resp, 3)
print(msg["result"]["content"][0]["text"])
```

---

## MCP protocol flow

The MCP handshake over SSE (what any compliant client does):

```
Client                                    Server
  │                                         │
  │── GET /sse ────────────────────────────▶│  (auth header if enabled)
  │◀─ event: endpoint ─────────────────────│  data: /messages?sessionId=<uuid>
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":1,"method":"initialize",...}
  │◀─ 202 Accepted (empty body) ───────────│
  │◀─ event: message ──────────────────────│  {"id":1,"result":{serverInfo,...}}
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":2,"method":"notifications/initialized"}
  │◀─ 202 Accepted ────────────────────────│  (no response — it's a notification)
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":3,"method":"tools/list",...}
  │◀─ 202 Accepted ────────────────────────│
  │◀─ event: message ──────────────────────│  {"id":3,"result":{tools:[...]}}
  │                                         │
  │── POST /messages?sessionId=<uuid> ────▶│  {"id":4,"method":"tools/call",...}
  │◀─ 202 Accepted ────────────────────────│
  │◀─ event: message ──────────────────────│  {"id":4,"result":{content:[...]}}
  │                                         │
  │  (keep-alive pings every 15s)           │
```

Key points:

- **`POST /messages` always returns `202 Accepted` with an empty body.**
  The response comes on the SSE stream, matched by the `id` field.
- **Notifications** (no `id`, e.g. `notifications/initialized`) get no
  response — the `202` is all you get.
- **One session per `GET /sse`.** Open multiple sessions if you need
  parallelism; each is independent.
- **Sessions expire** after `--session-timeout` minutes of inactivity.
  Reconnect with a fresh `GET /sse` if you get `404` on a POST.

---

## Running the server

### systemd (Linux)

```ini
# /etc/systemd/system/firebird-mcp.service
[Unit]
Description=firebird-5-mcp (SSE mode)
After=network.target

[Service]
User=mcp
ExecStart=/usr/local/bin/firebird-5-mcp --sse --host 0.0.0.0 --port 9000 \
    --token-file /etc/mcp/firebird-token \
    --base-url http://192.168.1.50:9000
Restart=on-failure
# The config is at /home/mcp/.firebird-mcp/config.json (or set FIREBIRD_CONFIG)

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

> The binary is a single static file — no runtime dependencies. The
> `config.json` must be mounted/copied in, and database paths in it must be
> valid **inside the container** (mount the `.fdb` files as volumes).

### Reverse proxy (nginx)

If you want HTTPS or a friendly hostname in front of the server:

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
        # SSE: disable buffering and timeouts
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 1h;
    }
}
```

> **Important:** disable `proxy_buffering` and set a long
> `proxy_read_timeout` — SSE is a long-lived stream, and nginx will
> otherwise buffer the response or drop the connection.
>
> When using a reverse proxy, pass `--base-url https://mcp.example.com` so
> `download_url` fields point at the public URL.

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| `401 Unauthorized` on `GET /sse` | Auth is enabled — send `Authorization: Bearer <token>` on **every** request (`/sse`, `/messages`, `/files`) |
| `404 Session not found or expired` on POST | The session was reaped (inactivity > `--session-timeout`) or the `sessionId` is wrong. Open a fresh `GET /sse` and use the new `endpoint` |
| No `download_url` in tool responses | The file is outside the storage directory, **or** the base URL is unknown (wildcard `--host` without `--base-url`). Pass `--base-url http://<host>:<port>` |
| Browser can't connect (CORS error) | Check the server is running and the port is reachable. All endpoints send `Access-Control-Allow-Origin: *` — if you see a CORS error, the request likely never reached the server (firewall / wrong host) |
| `EventSource` can't send the auth header | `EventSource` doesn't support custom headers. Use `fetch()` with a manual SSE reader (see the [JavaScript client](#javascript-client)) |
| Connection drops after ~60s behind a proxy | The proxy is buffering or timing out the SSE stream. Disable buffering and raise `proxy_read_timeout` (see [nginx](#reverse-proxy-nginx)) |
| `413 Payload Too Large` on upload | File exceeds the 500 MB limit |
| `400 path traversal not allowed` | The `path`/`target` contains `..`, is absolute, or escapes the storage directory |
| Server binds but nothing listens on the expected port | Check `--port` (default `9000`) and that no other process holds it |

---

## References

| Document | What it covers |
|---|---|
| [`README.md`](README.md) | Full tool reference (all tools with parameters and examples) |
| [`README-PTBR.md`](README-PTBR.md) | Same, in Portuguese |
| [`README_COMMAND_LINE.md`](README_COMMAND_LINE.md) | stdio transport, JSON-RPC protocol, helper scripts |
| [`README_MIGRATIONS.md`](README_MIGRATIONS.md) | Data migration (Parquet/IPC/CSV/DBF/JSONL) |
| [`README_CODEGEN.md`](README_CODEGEN.md) | Code generation examples (all languages) |
| [`README_FOR_HUMANS.md`](README_FOR_HUMANS.md) | Use it as a plain CLI — no AI agent needed |
| [MCP spec `2024-11-05`](https://modelcontextprotocol.io/specification/2024-11-05) | The HTTP+SSE transport specification |

---

## License

See [LICENSE](LICENSE).
