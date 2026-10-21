# Firebird PowerToys — Web UI

Web interface for the [firebird-5-mcp](../) server. Svelte 5 (no SvelteKit) + Vite.

## Prerequisites

- Node.js 20+
- The MCP server running in SSE mode on `http://localhost:9000`:

  ```bash
  ./scripts/build.sh
  ~/.local/bin/firebird-5-mcp --sse --host 127.0.0.1 --port 9000
  ```

  The server URL is fixed in `vite.config.ts` (`MCP_BASE_URL`). Change it there
  if your server runs elsewhere.

## Development

```bash
npm install
npm run dev
```

Open http://localhost:5173.

## Production build

```bash
npm run build
```

Outputs to `dist/`. The build is static — the MCP URL is baked in at build
time (no runtime env vars).

## Type check

```bash
npm run check
```

## Features (v1)

- **Query page** — SQL editor (CodeMirror, Firebird dialect, keyword
  autocomplete), run queries, results grid with sort/filter/CSV export,
  pagination (100 rows/page), execution plan, DML result text.
- **Schema page** — browse tables, procedures, functions, domains, triggers,
  indexes, sequences. View table details (columns, indexes, constraints,
  triggers). Extract DDL for any object.

## Architecture

```
src/
  main.ts                 # Svelte mount
  App.svelte              # Router, layout, connection status
  app.css                 # App-local CSS (imports lib theme)
  i18n/
    index.ts              # t() — merges app + lib messages
    locales/en.ts         # English messages
    locales/pt-BR.ts      # Portuguese (Brazil) messages
  lib/
    mcp/
      client.ts           # MCP client over HTTP+SSE (fetch + manual SSE parsing)
      connection.svelte.ts# Module-level connection state ($state)
      store.svelte.ts     # Databases cache + createAction() helper ($state)
      tools.ts            # Typed wrappers around firebird_* tools
    SqlEditor.svelte      # CodeMirror 6 editor (Firebird dialect)
    DbSelector.svelte     # Database dropdown (loads via firebird_get_databases)
    sqlKeywords.ts        # SQL keyword list for autocomplete
  pages/
    QueryPage.svelte      # SQL editor + results grid + pagination
    SchemaPage.svelte     # Object browser + table details + DDL extract
```

### MCP client

The client uses `fetch` + manual SSE stream reading (not `EventSource`) so
that auth headers (`Authorization: Bearer`) can be added later without rework.

Protocol (spec `2024-11-05`):

1. `GET {base}/sse` → SSE stream. First event is `endpoint` with
   `data: /messages?sessionId=<uuid>`.
2. `POST {base}{endpoint}` with JSON-RPC 2.0 body → `202 Accepted`.
3. Response arrives as SSE `message` event with matching `id`.

### State management

Svelte 5 runes only — no external state library. Module-level `$state` for
connection and databases; `createAction()` helper for per-component loading/
error/data.

## Phase 2 (planned)

- Import/export with grid preview
- Drop/edit objects
- Autocomplete with table/column names
- Protobuf/JSON export format
- Connection/token configuration page
