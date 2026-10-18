program test_migration;

{
  test_migration — Example of using firebird-5-mcp migration tools from Free Pascal.
  Demonstrates exporting a table to Parquet, describing the file, and importing it back.

  Build:
    fpc -otest_migration test_migration.pas

  Run:
    ./test_migration
}

{$mode objfpc}{$H+}

uses
  SysUtils,
  uMCP;

const
  DB = 'test-mcp';
  PARQUET_PATH = '/tmp/users_test.parquet';

procedure PrintSection(const ATitle: string);
begin
  Writeln;
  Writeln('========================================');
  Writeln(ATitle);
  Writeln('========================================');
end;

begin
  Writeln('firebird-5-mcp — Migration Tools Example');
  Writeln('========================================');

  { 1. Export USERS table to Parquet }
  PrintSection('1. Export Table to Parquet (firebird_export)');
  Writeln(MCPCall('firebird_export',
    Format('{"source":"USERS","output_path":"%s","format":"parquet","database":"%s"}',
      [PARQUET_PATH, DB])));

  { 2. Describe the exported Parquet file (no database connection needed) }
  PrintSection('2. Describe Parquet File (firebird_describe_file)');
  Writeln(MCPCall('firebird_describe_file',
    Format('{"path":"%s","format":"parquet"}', [PARQUET_PATH])));

  { 3. Import the Parquet file into a new table (mode: create) }
  PrintSection('3. Import Parquet to New Table (firebird_import, mode=create)');
  Writeln(MCPCall('firebird_import',
    Format('{"input_path":"%s","table":"USERS_MIGRATION_TEST","mode":"create","format":"parquet","database":"%s"}',
      [PARQUET_PATH, DB])));

  { 4. Verify the import — count rows in the new table }
  PrintSection('4. Verify Import (firebird_count)');
  Writeln(MCPCall('firebird_count',
    Format('{"table":"USERS_MIGRATION_TEST","database":"%s"}', [DB])));

  { 5. Query the imported table }
  PrintSection('5. Query Imported Table (firebird_query)');
  Writeln(MCPCall('firebird_query',
    Format('{"sql":"SELECT ID, NAME, EMAIL FROM USERS_MIGRATION_TEST","database":"%s"}', [DB])));

  { 6. Cleanup — drop the test table }
  PrintSection('6. Cleanup (firebird_drop)');
  Writeln(MCPCall('firebird_drop',
    Format('{"type":"TABLE","name":"USERS_MIGRATION_TEST","database":"%s"}', [DB])));

  Writeln;
  Writeln('Migration example completed.');
end.
