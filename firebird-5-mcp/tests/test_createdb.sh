#!/bin/sh
# Tests firebird_create_database with default page size, explicit page size,
# and an invalid page size (expecting an error).
#
# Usage:
#   sh tests/test_createdb.sh
#
# NOTE: this creates real database files under the allowed path
# (/database). Run tests/test_cleanup.sh afterwards to remove them.
BIN=/home/edujed/github/plamenix/jed-personal-mcp/bin/firebird-5-mcp-linux-amd64
export FIREBIRD_CONFIG=/home/edujed/github/plamenix/jed-personal-mcp/config.json

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"/database/ps_default.fdb"}}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"/database/ps_4k.fdb","page_size":"4096"}}}'
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"/database/ps_bad.fdb","page_size":"1234"}}}'
} | timeout 60 "$BIN" 2>/dev/null | grep -E '"id":[234]'
