# mcp-migration — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-migration`. Read this first to
> locate a function before opening the full source.
>
> **What this crate is:** Data file export/import for Firebird databases.
> Exposes three MCP tools: `firebird_export` (table/view/SQL → file),
> `firebird_import` (file → table, with `create`/`append` modes + `ignore_columns`),
> and `firebird_describe_file` (file schema without loading data).
> The public API is format-agnostic (`format` param); supports **Parquet**
> (Snappy default), **Arrow IPC / Feather v2** (ZSTD default), **CSV**
> (via `csv` crate v1.3), and **DBF** (dBASE/FoxPro/Clipper, via `dbase` crate
> v0.8). CSV import infers column types from a sample of the first 100 rows.
> DBF import uses the file header schema (no inference). Compression is
> configurable per format and auto-detected on import (from file extension
> for CSV/DBF, from file header for Parquet/IPC). Codepage is configurable
> for CSV and DBF (DBF restricted to CP437/CP850).

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 23 | Package `firebird-5-mcp-migration`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-types`, `arrow` (ipc_compression), `parquet` (snap+lz4+zstd), `csv`, `dbase` (yore+encoding_rs), `encoding_rs`, `yore`, `flate2`, `zstd`, `lz4_flex`, `serde_json`, `chrono` |
| `src/lib.rs` | 93 | Module declarations + re-exports + `import()` format-dispatch wrapper |
| `src/format.rs` | 225 | `Format` + `Compression` + `CodePage` enums, compression detection from extension |
| `src/io.rs` | 172 | `TextReader`/`TextWriter` (compression-aware I/O for CSV/DBF) |
| `src/types.rs` | 167 | Firebird ↔ Arrow ↔ DBF type mapping |
| `src/value.rs` | 56 | Firebird `Value` → primitive conversion helpers (text, int, float, bool) |
| `src/arrow_json.rs` | 132 | Arrow array → `serde_json::Value` conversion (for import) |
| `src/table.rs` | 196 | Table operations: existence check, creation (Arrow/CSV/DBF), append verification |
| `src/export.rs` | 389 | Export engine: query → Arrow → file (Parquet/IPC/CSV/DBF) |
| `src/import.rs` | 252 | Import engine: Parquet/IPC → Firebird table (shared logic) |
| `src/csv.rs` | 213 | CSV import + type inference from sample |
| `src/dbf.rs` | 377 | DBF import, export, describe (dBASE/FoxPro/Clipper) |
| `src/describe.rs` | 162 | Describe: file schema (Parquet/IPC footer / CSV header / DBF header) |

## `src/format.rs`

| Item | Summary |
|---|---|
| `Format` enum | `Parquet` / `Ipc` / `Csv` / `Dbase` + `parse()`, `as_str()`, `default_compression()`, `valid_compressions()` |
| `Compression` enum | `Snappy` / `Zstd` / `Lz4` / `Gzip` / `None` + `parse()`, `as_str()` |
| `CodePage` enum | `Cp437` / `Cp850` / `Iso88591` / `Win1252` / `Utf8` + `parse()`, `as_str()`, `default_for()`, `valid_for()`, `validate_for()`, `encode()`, `decode()` |
| `file_compression_from_path()` | Detects codec from file extension (`.zst`, `.gz`, `.lz4`, plain). Shared by CSV and DBF |

**Codepage rules:**
- DBF: only `Cp437` and `Cp850` (single-byte DOS codepages). Default: `Cp437`.
- CSV: all five codepages. Default: `Utf8`.
- `encode()` uses `yore::code_pages::CP437/CP850.encode_lossy()` for DOS pages,
  `encoding_rs::WINDOWS_1252.encode()` for ISO/Win, raw bytes for UTF-8.
- `decode()` uses `.decode(bytes).into_owned()` (yore returns `Cow<str>`),
  `encoding_rs::WINDOWS_1252.decode()`, or `from_utf8_lossy()`.

## `src/io.rs`

| Item | Summary |
|---|---|
| `TextReader` | Opens a file with transparent decompression based on extension. `into_csv_reader()` → `csv::Reader`, `into_bytes()` → `Vec<u8>` (for DBF) |
| `TextWriter` | Creates a file with transparent compression based on extension. `into_csv_writer()` → `csv::Writer`, `write_bytes()` → raw bytes (for DBF), `finish()` → flushes encoder trailer |
| `TextDecoder` / `TextEncoder` | `pub` enums (Plain/Zstd/Gzip/Lz4) — must be public because they appear in return types |

## `src/types.rs`

| Function | Summary |
|---|---|
| `firebird_to_arrow()` | Maps RDB$FIELD_TYPE code → Arrow `DataType`. NUMERIC/DECIMAL → `Decimal128(p,s)`, INT128 → `Int64` (lossy), BLOB → `Binary`, temporal → Date32/Time64/Timestamp. **Not used by the export path** (kept for future RDB$-based export) |
| `arrow_to_firebird()` | Maps Arrow `DataType` → Firebird DDL type string (for `create` mode). `Decimal128(p,s)` → `NUMERIC(p,s)`, `Utf8` → `VARCHAR(255)`, `Binary` → `BLOB SUB_TYPE 1` |
| `column_meta_to_arrow()` | Builds Arrow `Schema` from `ColumnMeta` (type info from query results). NUMERIC/DECIMAL → `Float64` (detected via `ColumnMeta.is_numeric`). **This is what the export path uses** |
| `dbf_type_to_firebird()` | Maps DBF `FieldType` → Firebird DDL type string (for DBF import `create` mode). `Character` → `VARCHAR(len)`, `Numeric` → `DOUBLE PRECISION`, `Logical` → `BOOLEAN`, `Date` → `DATE`, `DateTime` → `TIMESTAMP` |
| `check_append_compatibility()` | Validates column count + names (case-insensitive, trimmed) match between Arrow schema and Firebird table |

## `src/value.rs`

| Function | Summary |
|---|---|
| `value_to_text()` | Renders `Value` as display string (Null → `""`, Binary → `"<BLOB>"`, temporal → ISO) |
| `int_or_zero()` | Coerces `Value` to `i64` (non-numeric → 0) |
| `float_or_zero()` | Coerces `Value` to `f64` (non-numeric → 0.0) |
| `bool_or_false()` | Coerces `Value` to `bool` (non-boolean → false) |

## `src/arrow_json.rs`

| Function | Summary |
|---|---|
| `arrow_value_to_json()` | Converts one Arrow array value → `serde_json::Value` (downcast per type). Binary → hex string, Date32 → "YYYY-MM-DD", Timestamp → "YYYY-MM-DD HH:MM:SS", Decimal128 → formatted string |
| `format_decimal()` | Formats unscaled `i128` + scale → decimal string |
| `downcast()` | `ArrayRef` → concrete array type via `as_any().downcast_ref()` |

## `src/table.rs`

| Function | Summary |
|---|---|
| `table_exists()` | Checks RDB$RELATIONS for table existence |
| `create_table_from_arrow()` | Creates table from Arrow schema (fails if exists). DDL via `arrow_to_firebird` |
| `create_table_from_types()` | Creates table from explicit column names + DDL type strings (CSV/DBF path). Adds `NOT NULL` if no empty values in sample |
| `verify_append()` | Gets table columns from RDB$, calls `check_append_compatibility` (Arrow schema path) |
| `verify_append_types()` | Gets table columns from RDB$, checks count + names match (explicit types path, used by DBF) |

## `src/export.rs`

| Function | Summary |
|---|---|
| `export()` | **Unified export engine.** Normalises source, executes via `query_unbounded()`, builds Arrow schema from `ColumnMeta`, dispatches to Parquet/IPC/CSV/DBF writer |
| `write_csv()` | Writes `QueryResult` to CSV via `TextWriter` (compression from extension, codepage encoding) |
| `normalise_source()` | If source starts with `SELECT`/`WITH` → raw SQL; otherwise → `SELECT * FROM "source"` |
| `build_record_batch()` | Builds `RecordBatch` from `QueryResult` using positional column matching |
| `build_column_array()` | Builds one Arrow array per `DataType` variant |
| `naive_dt_to_micros()` | Converts `NaiveDateTime` → microseconds since Unix epoch |
| `parse_decimal()` | Parses decimal string → unscaled `i128` for `Decimal128` arrays |
| `write_parquet()` | `ArrowWriter` → `BufWriter` → `File::create` with configurable codec |
| `write_ipc()` | `IpcFileWriter::try_new_with_options` → configurable compression |

## `src/import.rs`

| Item | Summary |
|---|---|
| `ImportMode` | `Create` / `Append` enum + `parse()` |
| `ImportResult` | Struct: `table`, `rows`, `columns`, `mode` |
| `import_parquet()` | Reads Parquet → `RecordBatch`es → delegates to `import_record_batches` |
| `import_ipc()` | Reads Arrow IPC (Feather v2, LZ4/ZSTD auto-detected) → `RecordBatch`es → delegates to `import_record_batches` |
| `import_record_batches()` | **Shared import logic.** Filters `ignore_columns` → mode check (create/append) → `insert_batch` per batch (only kept columns) |
| `batch_to_rows_filtered()` | Converts `RecordBatch` → `Vec<Vec<serde_json::Value>>` for `insert_batch`, including only columns at `keep_indices` |
| `insert_rows_in_batches()` | Shared helper: inserts an iterator of rows in batches of 500 |
| `keep_column_indices()` | Shared helper: computes which column indices to keep after `ignore_columns` filtering |

## `src/csv.rs`

| Function | Summary |
|---|---|
| `import_csv()` | Reads CSV via `TextReader` (compression-aware) + codepage decode. Infers types from first 100 rows. Creates/verifies table, inserts in batches of 500 |
| `infer_type()` | Infers Firebird DDL type from sample values: INTEGER → DOUBLE PRECISION → BOOLEAN → DATE → TIMESTAMP → VARCHAR(255) |
| `is_integer/float/boolean/date/timestamp()` | Type check predicates (empty string = compatible with all) |
| `csv_value_to_json()` | Empty string → `Null`, otherwise → `String` |

## `src/dbf.rs`

| Function | Summary |
|---|---|
| `arrow_to_dbf_field()` | Maps Arrow `DataType` → (DBF `FieldType`, length, decimal). Int16→N(5), Int32→N(10), Int64→N(15), Float32→F(15), Float64→N(15,4), Utf8→C(254), Boolean→L(1), Binary→C(254), Date32→D(8), Timestamp→T(8), Time64→C(8), Decimal128→N(min(p,15),s) |
| `import_dbase()` | Reads DBF via `TextReader` (compression-aware) + codepage. Schema from file header (no inference). Creates/verifies table, inserts in batches |
| `dbf_value_to_json()` | Converts DBF `FieldValue` → `serde_json::Value` for `insert_batch`. Character→String (trimmed), Numeric→f64, Logical→Bool, Date→"YYYY-MM-DD", DateTime→"YYYY-MM-DD HH:MM:SS" |
| `write_dbase()` | **DBF export.** Validates field names ≤10 chars (abort with list if not). Writes to temp file via `TableWriterBuilder` (codepage-encoded), reads back, appends `0x1A` terminator if missing (dbase 0.8.0 bug), writes to final path via `TextWriter` (compression from extension) |
| `value_to_dbf_field()` | Converts Firebird `Value` → DBF `FieldValue` based on Arrow `DataType`. Integers/floats→Numeric, Utf8→Character (truncated to 254), Boolean→Logical, Date32→Date, Timestamp→DateTime, Binary→Character (hex, truncated to 254) |
| `describe_dbase()` | Reads DBF header via `TextReader` + codepage. Returns column names, DBF types, Firebird types, row count. No data loaded |

**DBF constraints (dBASE 3/4/Clipper):**
- Field names: max **10 characters** (11th byte is reserved for field type). Export aborts with error listing offending columns + alias suggestion.
- Character fields: max **254 bytes** (not 255 — first byte indicates length in some implementations).
- Numeric fields: max **15 digits** (for LibreOffice/Excel compatibility; dBASE III allows 19 but parsers reject >15).
- Record size: max 4000 bytes (not currently enforced).
- Codepage: CP437 or CP850 only (single-byte DOS pages). Multi-byte encodings (UTF-8, ISO-8859-1, Win1252) would corrupt the file.
- `0x1A` terminator: dbase crate 0.8.0 `finalize()` sometimes omits it. We check and append if missing (logged via `mlog!`).
- Temp file: `TableWriter` needs `Write + Seek`, so we write to `{path}.tmp`, `finalize()`, read bytes, delete temp, then write to final path via `TextWriter` (for compression support).

## `src/describe.rs`

| Item | Summary |
|---|---|
| `FileDescription` | Struct: `path`, `format`, `rows` (i64), `columns`, `arrow_types`, `firebird_types`, `sample_rows` (empty for Parquet/IPC/DBF) |
| `describe_file()` | Format dispatch: "parquet" → `describe_parquet`; "ipc" → `describe_ipc`; "csv" → `describe_csv`; "dbf" → `describe_dbase` |
| `describe_parquet()` | `ParquetRecordBatchReaderBuilder::try_new` → schema + row count from footer. No data loaded |
| `describe_ipc()` | `IpcFileReader::try_new(file, None)` → schema + row count (sums batch rows). No data loaded |
| `describe_csv()` | `TextReader` (compression-aware) + codepage decode → header + first 5 sample rows + total row count |
| `schema_to_description()` | Builds (columns, arrow_types, firebird_types) triple from an Arrow schema |

## Conventions

- Error type is `String` (not `FbError`) — this crate is a leaf that formats
  errors for the MCP layer.
- **Unified export engine:** table, view, and raw SQL all go through the same
  path — `normalise_source()` → `query_unbounded()` → `column_meta_to_arrow()`
  → `build_record_batch()` → `write_parquet()` / `write_ipc()` / `write_csv()` /
  `write_dbase()`. No RDB$ introspection. Export uses `query_unbounded()` (no
  `FIRST` injection) so the file always contains the complete result set — the
  100/1000-row caps only apply to LLM-facing `firebird_query` calls.
- **Format/compression/codepage centralised in `format.rs`:** `Format`,
  `Compression`, and `CodePage` enums are the single source of truth. CSV/DBF
  compression is determined by file extension (`.csv`, `.csv.zst`, `.csv.gz`,
  `.csv.lz4`, `.dbf`, `.dbf.zst`, `.dbf.gz`, `.dbf.lz4`) —
  `file_compression_from_path()` is the only place that maps extensions to
  codecs. `TextReader`/`TextWriter` wrap the appropriate decoder/encoder
  transparently.
- **Table operations centralised in `table.rs`:** `table_exists()`,
  `create_table_from_arrow()`, `create_table_from_types()`, `verify_append()`,
  and `verify_append_types()` are the only places that query RDB$RELATIONS /
  RDB$RELATION_FIELDS for import.
- **IPC (Feather v2):** export writes with ZSTD compression (default) or
  configurable via `compression` param. Import auto-detects LZ4 and ZSTD from
  the file header (no user config needed).
- **Shared import logic:** `import_record_batches()` in `import.rs` is the common
  path for Parquet and IPC imports (keep_indices, filtered schema, create/append,
  insert_batch). `import_parquet` and `import_ipc` are thin wrappers that read
  the file and delegate. `insert_rows_in_batches()` and `keep_column_indices()`
  are shared helpers used by all import paths (Parquet, IPC, CSV, DBF).
- **NUMERIC/DECIMAL → Float64:** the vendor patch renders NUMERIC/DECIMAL as
  exact decimal text (e.g. `"80.00"`), but the `raw_type` stays `SQL_INT64` (580),
  so `type_name` alone cannot distinguish a NUMERIC from a real BIGINT. The query
  layer (`mcp-core/src/query/mod.rs::rows_to_result`) detects this (raw_type 580 +
  decimal text) and sets `ColumnMeta.is_numeric = true`. The export path maps
  `is_numeric` → `Float64`, which preserves the value through the Arrow → JSON →
  Firebird import round-trip. Trade-off: loses exact decimal precision (e.g.
  `80.005` may become `80.004999...`), but the value is no longer zeroed out.
  For exact precision, the driver would need to propagate scale to `ColumnMeta`
  and the export would use `Decimal128`.
- `ArrayRef` is `Arc<dyn Array>` in Arrow 55 — use `Arc::new(arr) as ArrayRef`
  to construct, not `Box::new`.
- `RecordBatch::try_new` takes `Arc<Schema>` (not `&Schema`).
- `ParquetRecordBatchReaderBuilder` is at `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder`
  (not re-exported at `parquet::arrow` root).
- `builder.schema()` (Parquet) returns `&SchemaRef` (= `&Arc<Schema>`) — deref
  twice to get `&Schema`, then `.clone()` for an owned `Schema`.
- `reader.schema()` (IPC) returns `SchemaRef` (= `Arc<Schema>`) — deref once:
  `(*reader.schema()).clone()`.
- Binary data is exported as raw bytes (Parquet/IPC) or hex strings (CSV/DBF)
  and imported as hex strings (Firebird interprets hex strings as BLOB data on
  insert).
- INT128 → Int64 is lossy (truncated). Documented in `firebird_to_arrow`.
- RDB$ column names can have trailing spaces (CHAR padding) — always `.trim()`
  before comparison.
- `TextDecoder`/`TextEncoder` are `pub` (not private) because they appear in the
  return type of `TextReader::into_csv_reader()` / `TextWriter::into_csv_writer()`.
- **DBF codepage:** `yore` crate provides CP437/CP850 (DOS codepages).
  `encoding_rs` provides WINDOWS_1252 (for CSV). `yore::code_pages::CP437/CP850`
  are used directly in `dbf.rs` for the `dbase` crate's `ReaderBuilder` and
  `TableWriterBuilder`.
- **DBF `0x1A` terminator:** dbase crate 0.8.0 `finalize()` may omit the
  `0x1A` EOF byte. We check `data.ends_with(&[0x1A])` after reading the temp
  file and append it if missing (logged via `mlog!`). This is a workaround for
  an upstream bug.
