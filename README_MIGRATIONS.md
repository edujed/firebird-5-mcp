# Data Migration (Parquet / Arrow IPC / CSV / DBF / JSON Lines)

Tools for exporting and importing data between Firebird and data files (Parquet, Arrow IPC, CSV, DBF, JSON Lines).

## `firebird_export`

Exports a table, view, or SQL query to a data file. All three sources go through the same unified engine (table/view names are wrapped as `SELECT * FROM`).

**Arguments:**
- `source` (required): Table/view name (e.g. `'USERS'`) or a raw SQL `SELECT`/`WITH` statement
- `output_path` (required): Full path for the output file. Must be within an allowed path
- `format` (optional): Output format — `parquet` (default), `ipc`, `csv`, `dbf`, or `jsonl` (aliases: `ndjson`, `ldjson`)
- `compression` (optional): Compression codec — see table below
- `codepage` (optional): Character encoding — `cp437` (default for DBF), `cp850`, `iso-8859-1`, `windows-1252`, `utf-8` (default for CSV). For DBF, only `cp437` and `cp850` are valid (single-byte DOS codepages). JSONL is always UTF-8 (codepage ignored)
- `database` (optional): Database name (defaults to `default_database`)

**Compression options per format:**

| Format | Options | Default |
|--------|---------|---------|
| `parquet` | `snappy`, `zstd`, `lz4`, `none` | `snappy` |
| `ipc` | `zstd`, `lz4`, `none` | `zstd` |
| `csv` | determined by file extension (see below) | — |
| `dbf` | determined by file extension (see below) | — |
| `jsonl` | determined by file extension (see below) | — |

**CSV/DBF/JSONL compression by file extension:**

| Extension | Codec |
|-----------|-------|
| `.csv` / `.dbf` / `.jsonl` | none |
| `.csv.zst` / `.dbf.zst` / `.jsonl.zst` | ZSTD |
| `.csv.gz` / `.dbf.gz` / `.jsonl.gz` | GZIP |
| `.csv.lz4` / `.dbf.lz4` / `.jsonl.lz4` | LZ4 |

**Example (export table to Parquet with ZSTD):**
```json
{
  "source": "USERS",
  "output_path": "/tmp/users.parquet",
  "format": "parquet",
  "compression": "zstd",
  "database": "test-mcp"
}
```

**Example (export SQL query to Arrow IPC):**
```json
{
  "source": "SELECT ID, NAME, EMAIL FROM USERS WHERE ACTIVE = TRUE",
  "output_path": "/tmp/active_users.arrow",
  "format": "ipc",
  "database": "test-mcp"
}
```

**Example (export to compressed CSV):**
```json
{
  "source": "USERS",
  "output_path": "/tmp/users.csv.zst",
  "format": "csv",
  "database": "test-mcp"
}
```

**Example (export to DBF for LibreOffice/legacy tools):**
```json
{
  "source": "SELECT ID, NAME, CREATED_AT FROM USERS",
  "output_path": "/tmp/users.dbf",
  "format": "dbf",
  "database": "test-mcp"
}
```

**Example (export to JSON Lines for ETL pipelines):**
```json
{
  "source": "USERS",
  "output_path": "/tmp/users.jsonl",
  "format": "jsonl",
  "database": "test-mcp"
}
```

> **Note:** NUMERIC/DECIMAL columns are exported as `Float64` (approximate) in Parquet/IPC, and as exact decimal strings in CSV/JSONL. Binary data is exported as raw bytes (Parquet/IPC) or base64 strings (JSONL).
>
> **DBF note:** Output is strictly dBASE III (version `0x03`) for maximum compatibility with LibreOffice, Excel, and legacy tools. TIMESTAMP columns are stored as `Character(19)` with format `YYYY-MM-DD HH:MM:SS`. Field names must be ≤10 characters (use aliases in your SELECT if needed).
>
> **JSONL note:** Each row is a single JSON object on its own line. NULL → `null`, NUMERIC/DECIMAL → string (exact decimal), TIMESTAMP → `YYYY-MM-DD HH:MM:SS`, BLOB → base64 string. Always UTF-8.

## `firebird_import`

Imports a data file into a Firebird table. Two modes:
- **`create`** — Creates the table from the file schema (fails if it already exists), then inserts data
- **`append`** — Table must already exist with a compatible structure; inserts data into it

**Arguments:**
- `input_path` (required): Full path to the file to import
- `table` (required): Target table name
- `mode` (required): `create` or `append`
- `format` (optional): Input format — `parquet` (default), `ipc`, `csv`, `dbf`, or `jsonl` (aliases: `ndjson`, `ldjson`). For CSV/JSONL, column types are inferred from a sample of the first 100 rows. For DBF, types come from the file header (no inference)
- `ignore_columns` (optional): Array of file column names to skip (case-insensitive). Excluded from DDL in `create` mode; excluded from INSERT in `append` mode
- `codepage` (optional): Character encoding — `cp437` (default for DBF), `cp850`, `iso-8859-1`, `windows-1252`, `utf-8` (default for CSV). For DBF, only `cp437` and `cp850` are valid. JSONL is always UTF-8 (codepage ignored)
- `database` (optional): Database name (defaults to `default_database`)

> **Note:** Compression is auto-detected from the file header — no `compression` parameter needed on import.

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

**Example (import Arrow IPC, append to existing table):**
```json
{
  "input_path": "/tmp/new_employees.arrow",
  "table": "USERS",
  "mode": "append",
  "format": "ipc",
  "database": "test-mcp"
}
```

**Example (import compressed CSV, skip columns):**
```json
{
  "input_path": "/tmp/new_employees.csv.zst",
  "table": "USERS",
  "mode": "append",
  "format": "csv",
  "ignore_columns": ["SOURCE_FILE", "IMPORTED_AT"],
  "database": "test-mcp"
}
```

**Example (import DBF, create table):**
```json
{
  "input_path": "/tmp/employees.dbf",
  "table": "EMPLOYEES",
  "mode": "create",
  "format": "dbf",
  "database": "test-mcp"
}
```

**Example (import JSON Lines, create table):**
```json
{
  "input_path": "/incoming/events.jsonl.zst",
  "table": "EVENTS",
  "mode": "create",
  "format": "jsonl",
  "database": "test-mcp"
}
```

> **Note:** For CSV/JSONL import, column types are inferred from sample data: INTEGER → DOUBLE PRECISION → BOOLEAN → DATE → TIMESTAMP → VARCHAR(255) fallback.
>
> **DBF note:** The file header defines the schema — no type inference. Codepage defaults to `cp437` (use `codepage: "cp850"` for files encoded in CP850).
>
> **JSONL note:** Column names come from the keys of the first row (in file order). JSON numbers map to INTEGER/DOUBLE PRECISION, booleans to BOOLEAN, and `YYYY-MM-DD` / `YYYY-MM-DD HH:MM:SS` strings to DATE/TIMESTAMP.

## `firebird_describe_file`

Describes a data file's schema without loading its data: column names, Arrow types, mapped Firebird types, and row count. No database connection needed.

**Arguments:**
- `path` (required): Full path to the file to describe
- `format` (optional): File format — `parquet` (default), `ipc`, `csv`, `dbf`, or `jsonl` (aliases: `ndjson`, `ldjson`). For CSV/JSONL, shows column names + first 5 sample rows. For DBF, shows column names + DBF types + mapped Firebird types
- `codepage` (optional): Character encoding — `cp437` (default for DBF), `cp850`, `iso-8859-1`, `windows-1252`, `utf-8` (default for CSV). For DBF, only `cp437` and `cp850` are valid. JSONL is always UTF-8 (codepage ignored)

**Example (describe Parquet file):**
```json
{
  "path": "/tmp/users.parquet",
  "format": "parquet"
}
```

**Example (describe Arrow IPC file):**
```json
{
  "path": "/tmp/users.arrow",
  "format": "ipc"
}
```

**Example (describe CSV file):**
```json
{
  "path": "/tmp/employees.csv",
  "format": "csv"
}
```

**Example (describe DBF file):**
```json
{
  "path": "/tmp/employees.dbf",
  "format": "dbf"
}
```

**Example (describe JSON Lines file):**
```json
{
  "path": "/tmp/events.jsonl",
  "format": "jsonl"
}
```

## Type Mapping

### Firebird → Arrow (Export)

| Firebird Type | Arrow Type | Notes |
|---------------|------------|-------|
| INTEGER | Int32 | |
| BIGINT | Int64 | |
| SMALLINT | Int16 | |
| NUMERIC/DECIMAL | Float64 | Approximate — loses exact decimal precision |
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

## CSV / JSONL Type Inference (Import)

When importing CSV or JSONL files, column types are inferred from a sample of the first 100 rows:

| Sample Values | Inferred Type |
|---------------|---------------|
| All integers | INTEGER |
| All numeric (with decimals) | DOUBLE PRECISION |
| All true/false | BOOLEAN |
| All valid dates (YYYY-MM-DD) | DATE |
| All valid timestamps (YYYY-MM-DD HH:MM:SS) | TIMESTAMP |
| Mixed or unparseable | VARCHAR(255) |

Empty strings (CSV) and `null` (JSONL) are compatible with all types (treated as NULL).

## DBF Type Mapping

### Firebird → DBF (Export)

| Firebird Type | DBF Type | Notes |
|---------------|----------|-------|
| SMALLINT | N(6,0) | |
| INTEGER | N(11,0) | |
| BIGINT | N(15,0) | |
| FLOAT | N(12,4) | |
| DOUBLE PRECISION | N(19,6) | |
| NUMERIC/DECIMAL | N(min(p,15), min(s,p-1)) | |
| BOOLEAN | L(1) | |
| VARCHAR/CHAR | C(254) | Truncated to 254 bytes |
| BLOB | C(254) | Hex-encoded |
| DATE | D(8) | YYYYMMDD |
| TIME | C(8) | HH:MM:SS |
| TIMESTAMP | C(19) | YYYY-MM-DD HH:MM:SS |

### DBF → Firebird (Import, `create` mode)

| DBF Type | Firebird Type |
|----------|---------------|
| C(len) | VARCHAR(len) |
| N(p,s) | DOUBLE PRECISION |
| F(len) | DOUBLE PRECISION |
| L(1) | BOOLEAN |
| D(8) | DATE |
| T(8) | TIMESTAMP |
| I(4) | INTEGER |
| M(4) | BLOB SUB_TYPE 1 |

> **Note:** DBF export is strictly dBASE III (version `0x03`). FoxPro-only types (T, I, F, B, Y) are not used in export — TIMESTAMP becomes `C(19)`. On import, FoxPro types are read and mapped to Firebird equivalents.

## Limitations

- **NUMERIC/DECIMAL → Float64:** Loses exact decimal precision (e.g. `80.005` may become `80.004999...`). The value is preserved but not bit-exact. In CSV/JSONL export the exact decimal string is preserved instead.
- **INT128 → Int64:** Lossy conversion (truncated) during export.
- **Binary data:** Exported as raw bytes (Parquet/IPC) or base64 strings (JSONL). Imported as hex strings (Firebird interprets hex strings as BLOB data on insert).
- **CSV/DBF/JSONL compression:** Determined by file extension, not the `compression` parameter.
- **DBF field names:** Max 10 characters. Export aborts with an error listing offending columns + alias suggestion.
- **DBF codepage:** CP437 or CP850 only. Characters without a mapping in the DOS code page are replaced with `?` (lossy encoding).
- **JSONL:** Always UTF-8. Column names come from the keys of the first row, so all rows should share the same key set (missing keys are imported as NULL).
