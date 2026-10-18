# Data Migration (Parquet/CSV)

Tools for exporting and importing data between Firebird and data files (Parquet/CSV).

## `firebird_export`

Exports a table, view, or SQL query to a data file. All three sources go through the same unified engine (table/view names are wrapped as `SELECT * FROM`).

**Arguments:**
- `source` (required): Table/view name (e.g. `'USERS'`) or a raw SQL `SELECT`/`WITH` statement
- `output_path` (required): Full path for the output file. Must be within an allowed path
- `format` (optional): Output format — `parquet` (default) or `csv`
- `database` (optional): Database name (defaults to `default_database`)

**Example (export table to Parquet):**
```json
{
  "source": "USERS",
  "output_path": "/tmp/users.parquet",
  "format": "parquet",
  "database": "test-mcp"
}
```

**Example (export SQL query to CSV):**
```json
{
  "source": "SELECT ID, NAME, EMAIL FROM USERS WHERE ACTIVE = TRUE",
  "output_path": "/tmp/active_users.csv",
  "format": "csv",
  "database": "test-mcp"
}
```

> **Note:** NUMERIC/DECIMAL columns are exported as text (exact decimal strings). Binary data is exported as raw bytes.

## `firebird_import`

Imports a data file into a Firebird table. Two modes:
- **`create`** — Creates the table from the file schema (fails if it already exists), then inserts data
- **`append`** — Table must already exist with a compatible structure; inserts data into it

**Arguments:**
- `input_path` (required): Full path to the file to import
- `table` (required): Target table name
- `mode` (required): `create` or `append`
- `format` (optional): Input format — `parquet` (default) or `csv`. For CSV, column types are inferred from a sample of the first 100 rows
- `ignore_columns` (optional): Array of file column names to skip (case-insensitive). Excluded from DDL in `create` mode; excluded from INSERT in `append` mode
- `database` (optional): Database name (defaults to `default_database`)

**Example (import Parquet, create table):**
```json
{
  "input_path": "/tmp/users.parquet",
  "table": "USERS_BACKUP",
  "mode": "create",
  "format": "parquet",
  "database": "test-mcp"
}
```

**Example (import CSV, append to existing table, skip columns):**
```json
{
  "input_path": "/tmp/new_employees.csv",
  "table": "USERS",
  "mode": "append",
  "format": "csv",
  "ignore_columns": ["SOURCE_FILE", "IMPORTED_AT"],
  "database": "test-mcp"
}
```

> **Note:** For CSV import, column types are inferred from sample data: INTEGER → DOUBLE PRECISION → BOOLEAN → DATE → TIMESTAMP → VARCHAR(255) fallback.

## `firebird_describe_file`

Describes a data file's schema without loading its data: column names, Arrow types, mapped Firebird types, and row count. No database connection needed.

**Arguments:**
- `path` (required): Full path to the file to describe
- `format` (optional): File format — `parquet` (default) or `csv`. For CSV, shows column names + first 5 sample rows

**Example (describe Parquet file):**
```json
{
  "path": "/tmp/users.parquet",
  "format": "parquet"
}
```

**Example (describe CSV file):**
```json
{
  "path": "/tmp/employees.csv",
  "format": "csv"
}
```

## Type Mapping

### Firebird → Arrow (Export)

| Firebird Type | Arrow Type | Notes |
|---------------|------------|-------|
| INTEGER | Int32 | |
| BIGINT | Int64 | |
| SMALLINT | Int16 | |
| NUMERIC/DECIMAL | Utf8 | Exported as exact decimal string |
| DOUBLE PRECISION | Float64 | |
| FLOAT | Float32 | |
| BOOLEAN | Boolean | |
| VARCHAR/CHAR | Utf8 | |
| BLOB | Binary | Raw bytes |
| DATE | Date32 | |
| TIME | Time64 | |
| TIMESTAMP | Timestamp | |

### Arrow → Firebird (Import, `create` mode)

| Arrow Type | Firebird Type |
|------------|---------------|
| Int16 | SMALLINT |
| Int32 | INTEGER |
| Int64 | BIGINT |
| Float32 | FLOAT |
| Float64 | DOUBLE PRECISION |
| Utf8 | VARCHAR(255) |
| Boolean | BOOLEAN |
| Binary | BLOB SUB_TYPE 1 |
| Date32 | DATE |
| Time64 | TIME |
| Timestamp | TIMESTAMP |
| Decimal128(p,s) | NUMERIC(p,s) |

## CSV Type Inference (Import)

When importing CSV files, column types are inferred from a sample of the first 100 rows:

| Sample Values | Inferred Type |
|---------------|---------------|
| All integers | INTEGER |
| All numeric (with decimals) | DOUBLE PRECISION |
| All true/false | BOOLEAN |
| All valid dates (YYYY-MM-DD) | DATE |
| All valid timestamps (YYYY-MM-DD HH:MM:SS) | TIMESTAMP |
| Mixed or unparseable | VARCHAR(255) |

Empty strings are compatible with all types (treated as NULL).

## Limitations

- **Export:** Currently only `parquet` format is fully supported. CSV export may have limitations.
- **INT128 → Int64:** Lossy conversion (truncated) during export.
- **NUMERIC/DECIMAL:** Exported as `Utf8` (exact decimal strings) because `ColumnMeta` doesn't carry precision/scale.
- **Binary data:** Exported as raw bytes, imported as hex strings (Firebird interprets hex strings as BLOB data on insert).
