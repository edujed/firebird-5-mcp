#!/bin/sh
# Tests the JSON Lines (jsonl) format on firebird_export, firebird_describe_file,
# and firebird_import (create + append + invalid format).
#
# Usage:
#   sh tests/test_jsonl.sh
#
# Requires the deployed binary, config.json, and the test-mcp database.
# Creates a JSONL file in the project root and a JSONL_TEST table in test-mcp.
# Run tests/test_jsonl_cleanup.sh afterwards to remove the table.
BIN="${HOME}/.local/bin/firebird-5-mcp"
OUT="${HOME}/.firebird-mcp/output"

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # Export USERS to plain JSONL
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_export","arguments":{"source":"USERS","database":"test-mcp","output_path":"'"$OUT"'/jsonl_test.jsonl","format":"jsonl"}}}'
  # Export USERS to ZSTD-compressed JSONL
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_export","arguments":{"source":"USERS","database":"test-mcp","output_path":"'"$OUT"'/jsonl_test.jsonl.zst","format":"jsonl"}}}'
  # Describe the plain JSONL file (no DB connection needed)
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_describe_file","arguments":{"path":"'"$OUT"'/jsonl_test.jsonl","format":"jsonl"}}}'
  # Import the compressed JSONL, creating the table
  echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"firebird_import","arguments":{"input_path":"'"$OUT"'/jsonl_test.jsonl.zst","database":"test-mcp","table":"JSONL_TEST","mode":"create","format":"jsonl"}}}'
  # Import again, appending to the existing table
  echo '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"firebird_import","arguments":{"input_path":"'"$OUT"'/jsonl_test.jsonl","database":"test-mcp","table":"JSONL_TEST","mode":"append","format":"jsonl"}}}'
  # Invalid format string — should FAIL
  echo '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"firebird_export","arguments":{"source":"USERS","database":"test-mcp","output_path":"'"$OUT"'/bad.jsonl","format":"json"}}}'
} | timeout 120 "$BIN" 2>/dev/null | grep -E '"id":[234567]' | cut -c1-400
