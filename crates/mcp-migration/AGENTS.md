# mcp-migration — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-migration`. Read this first to
> locate a function before opening the full source.
>
> **What this crate is:** Data file export/import for Firebird databases.
> Exposes three MCP tools: `firebird_export` (table/view/SQL → file),
> `firebird_import` (file → table, with `create`/`append` modes + `ignore_columns`),
> and `firebird_describe_file` (file schema without loading data).
> The public API is format-agnostic (`format` param); supports **Parquet**
> (via Apache Arrow v55) and **CSV** (via `csv` crate v1.3). CSV import
> infers column types from a sample of the first 100 rows.

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 17 | Package `firebird-5-mcp-migration`; deps: `firebird-5-mcp-core`, `firebird-5-mcp-types`, `arrow`, `parquet`, `csv`, `serde_json`, `chrono` |
| `src/lib.rs` | 64 | Module declarations + re-exports + `import()` format-dispatch wrapper |
| `src/types.rs` | 149 | Firebird ↔ Arrow type mapping |
| `src/export.rs` | 331 | Export: table/view/SQL → Parquet or CSV (unified engine) |
| `src/import.rs` | 361 | Import: Parquet → Firebird table (create/append + ignore_columns) |
| `src/csv.rs` | 293 | Import: CSV → Firebird table (type inference from sample) |
| `src/describe.rs` | 118 | Describe: file schema (Parquet footer / CSV header + 5 sample rows) |

## `src/types.rs`

| Function | Lines | Summary |
|---|---|---|
| `firebird_to_arrow()` | 14–49 | Maps RDB$FIELD_TYPE code → Arrow `DataType`. NUMERIC/DECIMAL → `Decimal128(p,s)`, INT128 → `Int64` (lossy), BLOB → `Binary`, temporal → Date32/Time64/Timestamp. **Not used by the export path** (kept for future use) |
| `arrow_to_firebird()` | 54–70 | Maps Arrow `DataType` → Firebird DDL type string (for `create` mode). `Decimal128(p,s)` → `NUMERIC(p,s)`, `Utf8` → `VARCHAR(255)`, `Binary` → `BLOB SUB_TYPE 1` |
| `firebird_schema_to_arrow()` | 73–82 | Builds Arrow `Schema` from RDB$ column tuples. **Not used by the export path** (kept for future use) |
| `column_meta_to_arrow()` | 89–112 | Builds Arrow `Schema` from `ColumnMeta` (type info from query results). NUMERIC/DECIMAL → `Utf8` (no precision/scale available). **This is what the export path uses** |
| `check_append_compatibility()` | 121–149 | Validates column count + names (case-insensitive, trimmed) match between Arrow schema and Firebird table |

## `src/export.rs`

| Function | Lines | Summary |
|---|---|---|
| `export()` | 45–78 | **Unified export engine.** Normalises source (table/view name → `SELECT * FROM "name"`, raw SQL → as-is), executes via `query_unbounded()` (no row cap — export must fetch ALL rows), builds Arrow schema from `ColumnMeta`, dispatches to Parquet or CSV writer |
| `write_csv()` | 81–103 | Writes `QueryResult` to CSV via `csv::Writer` (header + rows, auto-escaping) |
| `normalise_source()` | 109–117 | If source starts with `SELECT`/`WITH` → raw SQL; otherwise → `SELECT * FROM "source"` |
| `build_record_batch()` | 123–139 | Builds `RecordBatch` from `QueryResult` using positional column matching (Nth field ↔ Nth value) |
| `build_column_array()` | 142–234 | Builds one Arrow array per `DataType` variant (Int16/32/64, Float32/64, Utf8, Boolean, Binary, Date32, Time64, Timestamp, Decimal128) |
| `naive_dt_to_micros()` | 237–246 | Converts `NaiveDateTime` → microseconds since Unix epoch (manual calc: days-from-CE − 719163) |
| `parse_decimal()` | 249–264 | Parses decimal string → unscaled `i128` for `Decimal128` arrays |
| `write_parquet()` | 267–279 | `ArrowWriter` → `BufWriter` → `File::create` |
| Value helpers | 283–323 | `value_to_text`, `int_or_zero`, `float_or_zero`, `bool_or_false` |

## `src/import.rs`

| Function | Lines | Summary |
|---|---|---|
| `ImportMode` | 26–44 | `Create` / `Append` enum + `parse()` |
| `import_parquet()` | 56–125 | Reads Parquet → `RecordBatch`es → filters `ignore_columns` → mode check (create/append) → `insert_batch` per batch (only kept columns) |
| `create_table()` | 128–160 | Checks RDB$RELATIONS for existence (errors if exists), generates DDL from filtered Arrow schema via `arrow_to_firebird`, executes `CREATE TABLE` |
| `verify_append()` | 164–205 | Gets table columns from RDB$ (`RF.RDB$FIELD_NAME` for the column name, `F.RDB$FIELD_TYPE` for the type), calls `check_append_compatibility` on the filtered schema |
| `batch_to_rows_filtered()` | 209–230 | Converts `RecordBatch` → `Vec<Vec<serde_json::Value>>` for `insert_batch`, including only columns at `keep_indices` |
| `arrow_value_to_json()` | 233–320 | Converts one Arrow array value → `serde_json::Value` (downcast per type). Binary → hex string, Date32 → "YYYY-MM-DD", Timestamp → "YYYY-MM-DD HH:MM:SS", Decimal128 → formatted string |
| `format_decimal()` | 323–340 | Formats unscaled `i128` + scale → decimal string |
| `downcast()` | 343–347 | `ArrayRef` → concrete array type via `as_any().downcast_ref()` |

## `src/csv.rs`

| Function | Lines | Summary |
|---|---|---|
| `import_csv()` | 25–145 | Reads CSV via `csv::ReaderBuilder` (flexible, has_headers). Infers types from first 100 rows. Creates/verifies table, inserts in batches of 500 |
| `create_table_csv()` | 148–175 | Checks RDB$RELATIONS, generates DDL from inferred types, executes `CREATE TABLE` |
| `verify_table_exists()` | 178–195 | Checks RDB$RELATIONS for table existence (append mode) |
| `infer_type()` | 198–220 | Infers Firebird DDL type from sample values: INTEGER → DOUBLE PRECISION → BOOLEAN → DATE → TIMESTAMP → VARCHAR(255) |
| `is_integer/float/boolean/date/timestamp()` | 223–260 | Type check predicates (empty string = compatible with all) |
| `csv_value_to_json()` | 263–270 | Empty string → `Null`, otherwise → `String` |

## `src/describe.rs`

| Function | Lines | Summary |
|---|---|---|
| `FileDescription` | 18–33 | Struct: `path`, `format`, `rows` (i64), `columns`, `arrow_types`, `firebird_types`, `sample_rows` (Vec of Vec, empty for Parquet) |
| `describe_file()` | 36–44 | Format dispatch: "parquet" → `describe_parquet`; "csv" → `describe_csv`; other → error |
| `describe_parquet()` | 46–72 | `ParquetRecordBatchReaderBuilder::try_new` → schema + row count from footer. No data loaded |
| `describe_csv()` | 74–110 | `csv::ReaderBuilder` (flexible, has_headers) → header + first 5 sample rows + total row count |

## Conventions

- Error type is `String` (not `FbError`) — this crate is a leaf that formats
  errors for the MCP layer.
- **Unified export engine:** table, view, and raw SQL all go through the same
  path — `normalise_source()` → `query_unbounded()` → `column_meta_to_arrow()`
  → `build_record_batch()` → `write_parquet()` / `write_csv()`. No RDB$
  introspection. Export uses `query_unbounded()` (no `FIRST` injection) so the
  file always contains the complete result set — the 100/1000-row caps only
  apply to LLM-facing `firebird_query` calls.
- NUMERIC/DECIMAL columns are exported as `Utf8` (the vendor patch renders
  them as exact decimal strings; `ColumnMeta` doesn't carry precision/scale).
- `ArrayRef` is `Arc<dyn Array>` in Arrow 55 — use `Arc::new(arr) as ArrayRef`
  to construct, not `Box::new`.
- `RecordBatch::try_new` takes `Arc<Schema>` (not `&Schema`).
- `ParquetRecordBatchReaderBuilder` is at `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder`
  (not re-exported at `parquet::arrow` root).
- `builder.schema()` returns `&SchemaRef` (= `&Arc<Schema>`) — deref twice to
  get `&Schema`, then `.clone()` for an owned `Schema`.
- Binary data is exported as raw bytes and imported as hex strings (Firebird
  interprets hex strings as BLOB data on insert).
- INT128 → Int64 is lossy (truncated). Documented in `firebird_to_arrow`.
- RDB$ column names can have trailing spaces (CHAR padding) — always `.trim()`
  before comparison.
