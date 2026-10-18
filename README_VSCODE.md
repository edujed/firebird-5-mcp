# VSCode & Cursor — MCP Configuration

## VSCode

### Option 1: Command Palette

1. Open the command palette (`Ctrl+Shift+P`)
2. Run: `MCP: Add Server`
3. Choose `Command (stdio)`
4. Insert the binary path (e.g. `/path/to/firebird-5-mcp`)
5. Name the server `firebird-5-mcp`
6. Choose `personal/user` or `project/workspace` scope
7. Trust the server when asked
8. Enable the tools in `Configure Tools`

### Option 2: `mcp-config.json`

Create or edit `.copilot/mcp-config.json` (project-level) or the user-level equivalent:

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

## Cursor

Create a `.cursor/mcp.json` file in your project root (or `~/.cursor/mcp.json` for global):

```json
{
  "mcpServers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "args": []
    }
  }
}
```

> **Note:** The binary resolves `config.json` automatically from `~/.firebird-mcp/config.json` (or `FIREBIRD_CONFIG` / `FIREBIRD_MCP_HOME` env vars). No environment variables are needed in the editor config unless you use a non-standard path.
