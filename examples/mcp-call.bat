@echo off
REM mcp-call.bat — Call a firebird-5-mcp tool from the command line (Windows).
REM
REM Usage:
REM   mcp-call.bat <tool_name> <json_arguments>
REM
REM Examples:
REM   mcp-call.bat firebird_list {"type":"TABLES","database":"test-mcp"}
REM   mcp-call.bat firebird_query {"sql":"SELECT * FROM USERS","database":"test-mcp"}
REM   mcp-call.bat firebird_describe_table {"table_name":"USERS","database":"test-mcp"}
REM   mcp-call.bat firebird_count {"table":"USERS","database":"test-mcp"}
REM   mcp-call.bat firebird_generate_dev {"tables":["USERS"],"language":"openapi","database":"test-mcp"}
REM   mcp-call.bat firebird_generate_dev {"tables":["USERS"],"language":"java","database":"test-mcp","outputDirPath":"C:\\tmp\\mcp-out"}
REM
REM Configuration:
REM   Set MCP_BINARY below, or override via environment:
REM   set MCP_BINARY=C:\path\to\firebird-5-mcp.exe
REM   (config.json is auto-loaded from %USERPROFILE%\.firebird-mcp\config.json by the binary)

setlocal

if "%MCP_BINARY%"=="" set MCP_BINARY=firebird-5-mcp.exe

if "%~1"=="" (
    echo Usage: %~nx0 ^<tool_name^> ^<json_arguments^>
    echo.
    echo Examples:
    echo   %~nx0 firebird_list {"type":"TABLES","database":"test-mcp"}
    echo   %~nx0 firebird_query {"sql":"SELECT * FROM USERS","database":"test-mcp"}
    echo   %~nx0 firebird_generate_dev {"tables":["USERS"],"language":"openapi","database":"test-mcp"}
    echo   %~nx0 firebird_generate_dev {"tables":["USERS"],"language":"java","database":"test-mcp","outputDirPath":"C:\\tmp\\mcp-out"}
    exit /b 1
)

set TOOL_NAME=%~1
set ARGS_JSON=%~2

set REQUEST={"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%TOOL_NAME%","arguments":%ARGS_JSON%}}

echo %REQUEST% | "%MCP_BINARY%" 2>nul | python -c "import sys,json; print(json.load(sys.stdin)['result']['content'][0]['text'])"

endlocal
