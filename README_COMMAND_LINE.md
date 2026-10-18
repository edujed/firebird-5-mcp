# Command Line Usage (No IDE Required)

The MCP server speaks JSON-RPC over stdio. You can call any tool directly from the shell — useful for scripting, CI pipelines, or IDEs without MCP support.

## How it works

1. The JSON-RPC request is piped to the server's **stdin**
2. The server processes the tool call and writes the JSON-RPC response to **stdout**
3. When stdin reaches EOF, the server exits
4. `fpc` or `python3` extracts the `result.content[0].text` field from the response

> **Note:** The `timeout 10` prevents hanging if the server doesn't exit. The `2>/dev/null` suppresses stderr (diagnostics).

## Helper scripts

Ready-to-use scripts are in [`examples/`](examples/):

| Script | Platform | Usage |
|--------|----------|-------|
| [`mcp-call.sh`](examples/mcp-call.sh) | Linux/macOS | `./mcp-call.sh <tool> '<json_args>'` |
| [`mcp-call.bat`](examples/mcp-call.bat) | Windows | `mcp-call.bat <tool> <json_args>` |

### Examples (Linux/macOS)

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
