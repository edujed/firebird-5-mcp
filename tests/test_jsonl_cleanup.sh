#!/bin/sh
# Removes the JSONL_TEST table created by tests/test_jsonl.sh.
#
# Usage:
#   sh tests/test_jsonl_cleanup.sh
BIN="${HOME}/.local/bin/firebird-5-mcp"

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_drop","arguments":{"database":"test-mcp","type":"TABLE","name":"JSONL_TEST"}}}'
} | timeout 60 "$BIN" 2>/dev/null | grep -E '"id":2' | cut -c1-400
