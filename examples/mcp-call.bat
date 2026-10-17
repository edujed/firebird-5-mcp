@echo off
REM mcp-call.bat — Call a firebird-5-mcp tool from the command line (Windows).
REM
REM Usage:
REM   mcp-call.bat <tool_name> <json_arguments>
REM
REM Examples:
REM   mcp-call.bat firebird_list {"type":"TABLES"}
REM   mcp-call.bat firebird_query {"sql":"SELECT * FROM USER"}
REM   mcp-call.bat firebird_describe_table {"table_name":"USER"}
REM   mcp-call.bat firebird_count {"table":"USER"}
REM   mcp-call.bat firebird_generate_dev {"tables":["USER"],"language":"openapi"}
REM
REM Configuration:
REM   Set MCP_BINARY and MCP_CONFIG below, or override via environment:
REM   set MCP_BINARY=C:\path\to\firebird-5-mcp.exe
REM   set MCP_CONFIG=C:\path\to\config.json

setlocal

if "%MCP_BINARY%"=="" set MCP_BINARY=C:\path\to\firebird-5-mcp.exe
if "%MCP_CONFIG%"=="" set MCP_CONFIG=C:\path\to\config.json

if "%~1"=="" (
    echo Usage: %~nx0 ^<tool_name^> ^<json_arguments^>
    echo.
    echo Examples:
    echo   %~nx0 firebird_list {"type":"TABLES"}
    echo   %~nx0 firebird_query {"sql":"SELECT * FROM USER"}
    echo   %~nx0 firebird_generate_dev {"tables":["USER"],"language":"openapi"}
    exit /b 1
)

set TOOL_NAME=%~1
set ARGS_JSON=%~2

set REQUEST={"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%TOOL_NAME%","arguments":%ARGS_JSON%}}

echo %REQUEST% | "%MCP_BINARY%" 2>nul | python -c "import sys,json; print(json.load(sys.stdin)['result']['content'][0]['text'])"

endlocal
