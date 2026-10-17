#!/bin/sh
# Tests the isolation/lock_wait parameters on firebird_run_script.
#
# Usage:
#   sh tests/test_isolation.sh
#
# Requires the deployed binary and config.json (see README.md).
BIN="${HOME}/.local/bin/firebird-5-mcp"

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # Default (no isolation param) — should use read-committed
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_run_script","arguments":{"database":"teste","script":"CREATE TABLE ISO_TEST (ID INTEGER); DROP TABLE ISO_TEST;"}}}'
  # Snapshot isolation
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_run_script","arguments":{"database":"teste","script":"CREATE TABLE ISO_TEST (ID INTEGER); DROP TABLE ISO_TEST;","isolation":"snapshot"}}}'
  # Read-only isolation — should FAIL because the script writes
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_run_script","arguments":{"database":"teste","script":"CREATE TABLE ISO_TEST (ID INTEGER);","isolation":"readonly"}}}'
  # Invalid isolation value
  echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"firebird_run_script","arguments":{"database":"teste","script":"SELECT 1;","isolation":"bogus"}}}'
  # With lock_wait + consistency
  echo '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"firebird_run_script","arguments":{"database":"teste","script":"CREATE TABLE ISO_TEST (ID INTEGER); DROP TABLE ISO_TEST;","isolation":"consistency","lock_wait":"5"}}}'
} | timeout 60 "$BIN" 2>/dev/null | grep -E '"id":[23456]' | cut -c1-400
