# mcp-migration — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-migration`. Read this first to
> locate a function before opening the full source.
>
> **What this crate is:** Data file export/import for Firebird databases.
> Exposes three MCP tools: `firebird_export` (table/view/SQL → file),
> `firebird_import` (file → table, with `create`/`append` modes + `ignore_columns`),
> and `firebird_describe_file` (file schema without loading data).
> The public API is format-agnostic (`format` param); supports **Parquet**
> (Snappy default), **Arrow IPC / Feather v2** (ZSTD default), and **CSV**
> (via `csv` crate v1.3). CSV import infers column types from a sample of
> the first 100 rows. Compression is configurable per format and auto-detected
> on import.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 20 | Package `firebird-5-mcp-migration`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-types`, `arrow` (ipc_compression), `parquet` (snap+lz4+zstd), `csv`, `flate2`, `zstd`, `lz4_flex`, `serde_json`, `chrono` |
| `src/lib.rs` | 75 | Module declarations + re-exports + `import()` format-dispatch wrapper |
| `src/format.rs` | 260 | `Format` + `Compression` enums, CSV compression detection from extension, `CsvReader`/`CsvWriter` (compression-aware I/O) |
| `src/types.rs` | 148 | Firebird ↔ Arrow type mapping (NUMERIC → Float64) |
| `src/value.rs` | 55 | Firebird `Value` → primitive conversion helpers (text, int, float, bool) |
| `src/arrow_json.rs` | 130 | Arrow array → `serde_json::Value` conversion (for import) |
| `src/table.rs` | 120 | Table operations: existence check, creation (Arrow/CSV), append verification |
| `src/export.rs` | 310 | Export engine: query → Arrow → file (Parquet/IPC/CSV) |
| `src/import.rs` | 200 | Import engine: Parquet/IPC → Firebird table (shared logic) |
| `src/csv.rs` | 200 | CSV import + type inference from sample |
| `src/describe.rs` | 130 | Describe: file schema (Parquet/IPC footer / CSV header + 5 sample rows) |

## `src/format.rs`

| Item | Summary |
|---|---|
| `Format` enum | `Parquet` / `Ipc` / `Csv` + `parse()`, `as_str()`, `default_compression()`, `valid_compressions()` |
| `Compression` enum | `Snappy` / `Zstd` / `Lz4` / `Gzip` / `None` + `parse()`, `as_str()` |
| `csv_compression_from_path()` | Detects CSV codec from file extension (`.csv.zst`, `.csv.gz`, `.csv.lz4`, plain) |
| `CsvReader` | Opens a CSV file with transparent decompression based on extension. `into_csv_reader()` → `csv::Reader` |
| `CsvWriter` | Creates a CSV file with transparent compression based on extension. `into_csv_writer()` → `csv::Writer`, `finish()` → flushes encoder trailer |

## `src/types.rs`

| Function | Summary |
|---|---|
| `firebird_to_arrow()` | Maps RDB$FIELD_TYPE code → Arrow `DataType`. NUMERIC/DECIMAL → `Decimal128(p,s)`, INT128 → `Int64` (lossy), BLOB → `Binary`, temporal → Date32/Time64/Timestamp. **Not used by the export path** (kept for future RDB$-based export) |
| `arrow_to_firebird()` | Maps Arrow `DataType` → Firebird DDL type string (for `create` mode). `Decimal128(p,s)` → `NUMERIC(p,s)`, `Utf8` → `VARCHAR(255)`, `Binary` → `BLOB SUB_TYPE 1` |
| `column_meta_to_arrow()` | Builds Arrow `Schema` from `ColumnMeta` (type info from query results). NUMERIC/DECIMAL → `Float64` (detected via `ColumnMeta.is_numeric`). **This is what the export path uses** |
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
| `create_table_from_types()` | Creates table from explicit column names + DDL type strings (CSV path). Adds `NOT NULL` if no empty values in sample |
| `verify_append()` | Gets table columns from RDB$, calls `check_append_compatibility` |

## `src/export.rs`

| Function | Summary |
|---|---|
| `export()` | **Unified export engine.** Normalises source, executes via `query_unbounded()`, builds Arrow schema from `ColumnMeta`, dispatches to Parquet/IPC/CSV writer |
| `write_csv()` | Writes `QueryResult` to CSV via `CsvWriter` (compression from extension) |
| `write_csv_body()` | Writes CSV header + rows to a `csv::Writer` |
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

## `src/csv.rs`

| Function | Summary |
|---|---|
| `import_csv()` | Reads CSV via `CsvReader` (compression-aware). Infers types from first 100 rows. Creates/verifies table, inserts in batches of 500 |
| `infer_type()` | Infers Firebird DDL type from sample values: INTEGER → DOUBLE PRECISION → BOOLEAN → DATE → TIMESTAMP → VARCHAR(255) |
| `is_integer/float/boolean/date/timestamp()` | Type check predicates (empty string = compatible with all) |
| `csv_value_to_json()` | Empty string → `Null`, otherwise → `String` |

## `src/describe.rs`

| Item | Summary |
|---|---|
| `FileDescription` | Struct: `path`, `format`, `rows` (i64), `columns`, `arrow_types`, `firebird_types`, `sample_rows` (empty for Parquet/IPC) |
| `describe_file()` | Format dispatch: "parquet" → `describe_parquet`; "ipc" → `describe_ipc`; "csv" → `describe_csv` |
| `describe_parquet()` | `ParquetRecordBatchReaderBuilder::try_new` → schema + row count from footer. No data loaded |
| `describe_ipc()` | `IpcFileReader::try_new(file, None)` → schema + row count (sums batch rows). No data loaded |
| `describe_csv()` | `CsvReader` (compression-aware) → header + first 5 sample rows + total row count |
| `schema_to_description()` | Builds (columns, arrow_types, firebird_types) triple from an Arrow schema |

## Conventions

- Error type is `String` (not `FbError`) — this crate is a leaf that formats
  errors for the MCP layer.
- **Unified export engine:** table, view, and raw SQL all go through the same
  path — `normalise_source()` → `query_unbounded()` → `column_meta_to_arrow()`
  → `build_record_batch()` → `write_parquet()` / `write_ipc()` / `write_csv()`. No RDB$
  introspection. Export uses `query_unbounded()` (no `FIRST` injection) so the
  file always contains the complete result set — the 100/1000-row caps only
  apply to LLM-facing `firebird_query` calls.
- **Format/compression centralised in `format.rs`:** `Format` and `Compression`
  enums are the single source of truth. CSV compression is determined by file
  extension (`.csv`, `.csv.zst`, `.csv.gz`, `.csv.lz4`) — `csv_compression_from_path()`
  is the only place that maps extensions to codecs. `CsvReader`/`CsvWriter`
  wrap the appropriate decoder/encoder transparently.
- **Table operations centralised in `table.rs`:** `table_exists()`,
  `create_table_from_arrow()`, `create_table_from_types()`, and `verify_append()`
  are the only places that query RDB$RELATIONS / RDB$RELATION_FIELDS for import.
- **IPC (Feather v2):** export writes with ZSTD compression (default) or
  configurable via `compression` param. Import auto-detects LZ4 and ZSTD from
  the file header (no user config needed).
- **Shared import logic:** `import_record_batches()` in `import.rs` is the common
  path for Parquet and IPC imports (keep_indices, filtered schema, create/append,
  insert_batch). `import_parquet` and `import_ipc` are thin wrappers that read
  the file and delegate.
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
- Binary data is exported as raw bytes and imported as hex strings (Firebird
  interprets hex strings as BLOB data on insert).
- INT128 → Int64 is lossy (truncated). Documented in `firebird_to_arrow`.
- RDB$ column names can have trailing spaces (CHAR padding) — always `.trim()`
  before comparison.
- `CsvDecoder`/`CsvEncoder` are `pub` (not private) because they appear in the
  return type of `CsvReader::into_csv_reader()` / `CsvWriter::into_csv_writer()`.
