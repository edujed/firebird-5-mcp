# JetBrains — MCP Configuration

Applies to: IntelliJ, GoLand, WebStorm, CLion, and other JetBrains IDEs with AI Assistant.

## Option 1: AI Assistant Settings

In the AI Assistant settings, add a new MCP server:

- **Command:** `/path/to/firebird-5-mcp`

## Option 2: MCP Config File

Edit the MCP config file (`~/.jetbrains/mcp.json` or project-level `.mcp.json`):

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
