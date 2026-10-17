#!/usr/bin/env bash
# mcp-call.sh — Call a firebird-5-mcp tool from the command line.
#
# Usage:
#   ./mcp-call.sh <tool_name> '<json_arguments>'
#
# Examples:
#   ./mcp-call.sh firebird_list '{"type":"TABLES"}'
#   ./mcp-call.sh firebird_query '{"sql":"SELECT * FROM USER"}'
#   ./mcp-call.sh firebird_describe_table '{"table_name":"USER"}'
#   ./mcp-call.sh firebird_count '{"table":"USER"}'
#   ./mcp-call.sh firebird_generate_dev '{"tables":["USER"],"language":"openapi"}'
#
# Configuration:
#   Set MCP_BINARY and MCP_CONFIG below, or override via environment:
#   MCP_BINARY=/path/to/firebird-5-mcp MCP_CONFIG=/path/to/config.json ./mcp-call.sh ...

set -euo pipefail

MCP_BINARY="${MCP_BINARY:-/path/to/firebird-5-mcp}"
MCP_CONFIG="${MCP_CONFIG:-/path/to/config.json}"

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <tool_name> '<json_arguments>'" >&2
  echo "" >&2
  echo "Examples:" >&2
  echo "  $0 firebird_list '{\"type\":\"TABLES\"}'" >&2
  echo "  $0 firebird_query '{\"sql\":\"SELECT * FROM USER\"}'" >&2
  echo "  $0 firebird_generate_dev '{\"tables\":[\"USER\"],\"language\":\"openapi\"}'" >&2
  exit 1
fi

TOOL_NAME="$1"
ARGS_JSON="$2"

REQUEST=$(printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}' \
  "$TOOL_NAME" "$ARGS_JSON")

echo "$REQUEST" \
  | FIREBIRD_CONFIG="$MCP_CONFIG" timeout 10 "$MCP_BINARY" 2>/dev/null \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['content'][0]['text'])"
