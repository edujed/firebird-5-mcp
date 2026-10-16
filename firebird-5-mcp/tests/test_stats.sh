#!/bin/sh
# Tests the firebird_crypt_state and firebird_database_stats tools.
#
# Usage:
#   sh tests/test_stats.sh
#
# Requires the deployed binary and config.json (see README.md).
BIN=/home/edujed/github/plamenix/jed-personal-mcp/bin/firebird-5-mcp-linux-amd64
export FIREBIRD_CONFIG=/home/edujed/github/plamenix/jed-personal-mcp/config.json

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_crypt_state","arguments":{}}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_database_stats","arguments":{}}}'
} | timeout 60 "$BIN" 2>/dev/null | grep -E '"id":[23]'
