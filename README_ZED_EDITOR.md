# Zed Editor — MCP Configuration

Add the `firebird-5-mcp` server to your Zed settings (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "firebird-5-mcp": {
      "command": "/path/to/firebird-5-mcp",
      "enabled": true,
      "timeout": 30000
    }
  }
}
```

> **Note:** The binary resolves `config.json` automatically from `~/.firebird-mcp/config.json` (or `FIREBIRD_CONFIG` / `FIREBIRD_MCP_HOME` env vars). No environment variables are needed in the editor config unless you use a non-standard path.
