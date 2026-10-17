@echo off
rem Tests pagination (max_rows/skip) and the explain_plan injection fix.
rem
rem Usage:
rem   tests\test_pagination.bat
rem
rem Requires the deployed binary and config.json (see README.md).
setlocal
set "BIN=%~dp0..\..\bin\firebird-5-mcp-windows-amd64.exe"
set "FIREBIRD_CONFIG=%~dp0..\..\config.json"

(
  echo {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}
  echo {"jsonrpc":"2.0","method":"notifications/initialized"}
  echo {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT RDB$RELATION_NAME FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0 ORDER BY 1","max_rows":"3","skip":"0"}}}
  echo {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT RDB$RELATION_NAME FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0 ORDER BY 1","max_rows":"3","skip":"3"}}}
  echo {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_explain_plan","arguments":{"sql":"SELECT * FROM CARGO_FUNCAO WHERE NOME = 'x'"}}}
) | "%BIN%"
endlocal
