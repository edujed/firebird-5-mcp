@echo off
rem Tests firebird_create_database with default page size, explicit page size,
rem and an invalid page size (expecting an error).
rem
rem Usage:
rem   tests\test_createdb.bat
rem
rem NOTE: this creates real database files under the allowed path.
rem Run tests\test_cleanup.bat afterwards to remove them.
rem Adjust the "path" values below to match your config.json allowed_paths.
setlocal
set "BIN=%USERPROFILE%\.local\bin\firebird-5-mcp.exe"

(
  echo {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"shell-test","version":"1.0"}}}
  echo {"jsonrpc":"2.0","method":"notifications/initialized"}
  echo {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"C:\\database\\ps_default.fdb"}}}
  echo {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"C:\\database\\ps_4k.fdb","page_size":"4096"}}}
  echo {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"firebird_create_database","arguments":{"path":"C:\\database\\ps_bad.fdb","page_size":"1234"}}}
) | "%BIN%"
endlocal
