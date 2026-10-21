# mcp-types — Code Summary (for AI agents)

> **Purpose:** token-efficient map of `crates/mcp-types`. Read this first to
> locate a type or helper before opening the full source.
>
> **What this crate is:** shared value types for the Firebird MCP project.
> **No IO, no async, no driver code** — just data shapes, result containers,
> and pure formatting helpers. Consumed by `mcp-core`, `mcp-tools`, and both
> binary crates (stdio and the future HTTP/SSE edition).

## File map

| File | Lines | Role |
|---|---|---|
| `Cargo.toml` | 11 | Package `firebird-5-mcp-types`; deps: `chrono` |
| `src/lib.rs` | 306 | Everything: `Value`, `ColumnMeta`, `QueryResult`, type formatting, identifier validation, Markdown cell rendering, DRY helpers (`is_not_null`, `trigger_type_label`) |

## `src/lib.rs`

| Item | Lines | Summary |
|---|---|---|
| `enum Value` | 29–39 | Single column value: Null, Bool, Int, Float, Text, Date, Time, Timestamp, Binary |
| `struct ColumnMeta` | 43–54 | `{ name, type_name, is_numeric }` — `is_numeric` flags NUMERIC/DECIMAL columns (detected by the query layer) |
| `struct QueryResult` | 58–61 | `{ columns: Vec<ColumnMeta>, rows: Vec<Vec<Value>> }` |
| `QueryResult::get` | 63–68 | Value at row/col, Null if out of bounds |
| `QueryResult::col_index` | 71–75 | Column index by name (case-insensitive) |
| `Value::as_i64` | 80–87 | Coerce to i64 (parses text, bools) |
| `Value::as_text` | 90–96 | Coerce to String (text or int) |
| `FIREBIRD_TYPE_NAMES` | 107–132 | RDB$FIELD_TYPE number → Firebird type name (24 entries, verified vs Firebird 5) |
| `type_name_for` | 134–139 | Lookup helper |
| `format_column_type` | 148–198 | Human-readable type: NUMERIC/DECIMAL(p,s), CHAR/VARCHAR/NVARCHAR(n) bytes→chars, BLOB sub-type |
| `char_size_for` | 213–221 | NVARCHAR = /2 (2-byte units); CHAR/VARCHAR = as-is (already in characters) |
| `is_valid_identifier` | 261–268 | Letters/digits/underscore, must start with a letter |
| `is_not_null` | 231–239 | Checks if a column is NOT NULL from RDB$ metadata (single source of truth) |
| `trigger_type_label` | 243–250 | Maps `RDB$TRIGGER_TYPE` enum (1-6) to human-readable label (BEFORE/AFTER INSERT/UPDATE/DELETE) |
| `format_cell` | 276–294 | Markdown cell: NULL, flatten newlines, escape pipes, `<BLOB>` for binary |
| `format_float` | 296–302 | Whole floats render as integers |
| `flatten` | 304–306 | Newlines → space, pipes → `\|` |

## Conventions

- This crate is the **only** crate that may be a direct dependency of every
  other crate. Other crates should not depend on each other unless one is a
  domain trait host for the other.
- No IO, no async, no driver code, no `dyn` traits. If a candidate type needs
  a dependency beyond `serde`, `serde_json`, `chrono`, or `thiserror`, it
  belongs in `mcp-core` instead.
- `mcp-core` re-exports these types via its `types` module for backward
  compatibility — new code should depend on `mcp-types` directly.
