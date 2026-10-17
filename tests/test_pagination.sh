#!/bin/sh
# Tests pagination (max_rows/skip) and the explain_plan injection fix.
#
# Usage:
#   sh tests/test_pagination.sh
#
# Requires the deployed binary and config.json (see README.md).
BIN=/home/edujed/github/plamenix/jed-personal-mcp/bin/firebird-5-mcp-linux-amd64
export FIREBIRD_CONFIG=/home/edujed/github/plamenix/jed-personal-mcp/config.json

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT RDB$RELATION_NAME FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0 ORDER BY 1","max_rows":"3","skip":"0"}}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT RDB$RELATION_NAME FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0 ORDER BY 1","max_rows":"3","skip":"3"}}}'
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_explain_plan","arguments":{"sql":"SELECT * FROM CARGO_FUNCAO WHERE NOME = '"'"'x'"'"'"}}}'
} | timeout 60 "$BIN" 2>/dev/null | grep -E '"id":[234]' | cut -c1-1500
