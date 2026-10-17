@echo off
rem Tests the firebird_query_history tool.
rem
rem Usage:
rem   tests\test_query_history.bat
rem
rem Requires the deployed binary and config.json (see README.md).
setlocal
set "BIN=%USERPROFILE%\.local\bin\firebird-5-mcp.exe"

(
  echo {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}
  echo {"jsonrpc":"2.0","method":"notifications/initialized"}
  echo {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query_history","arguments":{}}}
  echo {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_query_history","arguments":{"state":"1"}}}
  echo {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_query_history","arguments":{"user":"SYSDBA"}}}
  echo {"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"firebird_query_history","arguments":{"limit":"5"}}}
) | "%BIN%"
endlocal
