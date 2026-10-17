program test_mcp;

{
  test_mcp — Command-line test for the firebird-5-mcp MCP server.
  Demonstrates calling MCP tools from Free Pascal without an IDE.

  Build:
    fpc -otest_mcp test_mcp.pas

  Run:
    ./test_mcp
}

{$mode objfpc}{$H+}

uses
  SysUtils,
  uMCP;

procedure PrintSection(const ATitle: string);
begin
  Writeln;
  Writeln('========================================');
  Writeln(ATitle);
  Writeln('========================================');
end;

begin
  Writeln('firebird-5-mcp — Free Pascal MCP Client Test');
  Writeln('============================================');

  { 1. List all available tools }
  PrintSection('1. Available Tools (tools/list)');
  Writeln(MCPListTools);

  { 2. List tables }
  PrintSection('2. List Tables (firebird_list)');
  Writeln(MCPCall('firebird_list', '{"type":"TABLES"}'));

  { 3. Describe a table }
  PrintSection('3. Describe Table (firebird_describe_table)');
  Writeln(MCPCall('firebird_describe_table', '{"table_name":"USER"}'));

  { 4. Query data }
  PrintSection('4. Query (firebird_query)');
  Writeln(MCPCall('firebird_query', '{"sql":"SELECT ID, NAME, ACTIVE FROM USER"}'));

  { 5. Count rows }
  PrintSection('5. Count (firebird_count)');
  Writeln(MCPCall('firebird_count', '{"table":"USER"}'));

  { 6. Generate code (Lazarus) }
  PrintSection('6. Code Gen — Lazarus (firebird_generate_dev)');
  Writeln(MCPCall('firebird_generate_dev', '{"tables":["USER"],"language":"lazarus"}'));

  { 7. Generate code (OpenAPI/Swagger) }
  PrintSection('7. Code Gen — OpenAPI/Swagger (firebird_generate_dev)');
  Writeln(MCPCall('firebird_generate_dev', '{"tables":["USER"],"language":"openapi"}'));

  Writeln;
  Writeln('All tests completed.');
end.
