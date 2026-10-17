# Free Pascal / Lazarus — MCP Client Example

Example of calling the `firebird-5-mcp` MCP server from a Free Pascal command-line program, without any IDE or AI integration.

## Files

| File | Description |
|------|-------------|
| `uMCP.pas` | MCP client unit — spawns the MCP binary via POSIX pipes (`fork`/`exec`/`read`/`write`), sends JSON-RPC requests, and parses the response with `fpjson`. |
| `test_mcp.pas` | Test program that exercises 7 MCP tools: `tools/list`, `firebird_list`, `firebird_describe_table`, `firebird_query`, `firebird_count`, `firebird_generate_dev` (Lazarus), and `firebird_generate_dev` (OpenAPI). |

## Requirements

- **Free Pascal** 3.2.2+ (tested on FPC 3.2.2 / Linux x86_64)
- **`fcl-json`** package (provides `fpjson` and `jsonparser` units) — included in standard FPC distributions
- **`firebird-5-mcp`** binary (built from the Rust project)
- **`config.json`** with at least one database configured

## Configuration

Edit the constants at the top of `uMCP.pas`:

```pascal
const
  MCP_BINARY  = '/path/to/firebird-5-mcp';
  MCP_CONFIG  = '/path/to/config.json';
```

## Build

```bash
fpc -otest_mcp test_mcp.pas
```

This compiles both `test_mcp.pas` and `uMCP.pas` (found in the same directory) into a single static binary.

## Run

```bash
./test_mcp
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
### Tables and Views (21)
...

========================================
3. Describe Table (firebird_describe_table)
========================================
### Table: USER
...

========================================
4. Query (firebird_query)
========================================
### Query Results (100 rows)
...

========================================
5. Count (firebird_count)
========================================
Count: 229 rows in USER

========================================
6. Code Gen — Lazarus (firebird_generate_dev)
========================================
object FormUser: TFormManutencao
...

========================================
7. Code Gen — OpenAPI/Swagger (firebird_generate_dev)
========================================
  User:
    type: object
    description: Schema for table USER
    required:
      - createdAt
      - updatedAt
      - salary
      - employeeId
    properties:
      id:
        type: integer
        format: int32
        nullable: true
      name:
        type: string
        nullable: true
      email:
        type: string
        nullable: true
      passwordHash:
        type: string
        nullable: true
      active:
        type: integer
        format: int32
        nullable: true
      createdAt:
        type: string
        format: date-time
      updatedAt:
        type: string
        format: date-time
      salary:
        type: number
        format: decimal
      employeeId:
        type: string
```

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
  Writeln(MCPCall('firebird_count', '{"table":"USER"}'));
end.
```

> **Note:** The `MCP_BINARY` and `MCP_CONFIG` constants are hardcoded in `uMCP.pas`. For a production app, consider making them configurable (e.g., via command-line parameters or an INI file).
