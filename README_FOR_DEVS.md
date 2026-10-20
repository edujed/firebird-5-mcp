# Developer Guide

## Prerequisites

- **Firebird 5** server running (or any version 3+)
- **Rust** toolchain

## Build

Use the build script from the project root:

```bash
./scripts/build.sh              # Current platform (binary → ~/.local/bin/firebird-5-mcp)
./scripts/build.sh all          # All supported platforms
```

Or build manually:

```bash
cargo build --release
# Binary will be at: target/release/firebird-5-mcp
```

### Cross-compilation notes

| Platform | Target | Requirements |
|---|---|---|
| linux/amd64 | `x86_64-unknown-linux-gnu` | None (native on x86_64) |
| linux/arm64 | `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu-gcc` (`sudo apt install gcc-aarch64-linux-gnu`) |
| windows/amd64 | `x86_64-pc-windows-gnu` | `x86_64-w64-mingw32-gcc` (`sudo apt install gcc-mingw-w64-x86-64`) |
| darwin/* | `*-apple-darwin` | [osxcross](https://github.com/tpoechtrager/osxcross) (skipped with a warning if not installed) |
| windows/arm64 | — | Not supported (no stable GNU target in Rust) |

> **Note:** The binary name is always `firebird-5-mcp` (no `-os-arch` suffix). Cross-compiled binaries overwrite each other in `~/.local/bin/` — this is intentional since local builds target the current platform only. Release artifacts with platform suffixes are built by GitHub Actions.

## Release

```bash
./scripts/publish_new_tag.sh v1.2.0 "feat: add new feature"
```

Commits, tags, and pushes to origin. GitHub Actions handles the rest.

## Project Structure

```
firebird-5-mcp/             # Repository root
├── Cargo.toml              # Workspace manifest
├── crates/
│   ├── mcp-types/          # Shared value types (Value, ColumnMeta, QueryResult)
│   ├── mcp-core/           # Core library — client, query, schema, DDL, metadata, stats, config, codegen
│   ├── mcp-tools/          # Shared MCP tool handlers (transport-agnostic)
│   ├── mcp-stdio-bin/      # MCP stdio server binary (firebird-5-mcp)
│   └── mcp-migration/      # Data file export/import (Parquet/IPC/CSV/DBF/JSONL)
├── vendor/
│   └── rsfbclient-rust/    # Vendored driver with NUMERIC/DECIMAL precision patch
├── scripts/
│   ├── build.sh            # Multi-platform build
│   └── publish_new_tag.sh  # Tag + release helper
├── examples/
│   ├── test-mcp/           # Complete test database
│   ├── freePascal/         # Free Pascal MCP client example
│   ├── mcp-call.sh         # CLI helper (Linux/macOS)
│   └── mcp-call.bat        # CLI helper (Windows)
├── tests/                  # Shell smoke tests
├── .github/workflows/
│   └── release.yml         # CI/CD release pipeline
├── README.md
├── CHANGELOG.md
└── LICENSE
```

## Design

The project is split into five crates:

1. **`mcp-types`** — Shared value types (`Value`, `ColumnMeta`, `QueryResult`) + pure formatting helpers. No IO, no async, no driver code.

2. **`mcp-core`** — Core library with all Firebird logic: connection, query, schema, DDL, metadata, stats, storage, config, codegen. Can be used in other Rust projects without the MCP layer.

3. **`mcp-tools`** — Shared MCP tool handlers (transport-agnostic): handlers, `ToolResult`, `dispatch()`, `tool_definitions()`.

4. **`mcp-stdio-bin`** — Thin MCP-stdio layer: JSON-RPC protocol loop + config management. Depends on `mcp-tools`.

5. **`mcp-migration`** — Data file export/import: tables, views, SQL → Parquet/IPC/CSV/DBF/JSONL; Parquet/IPC/CSV/DBF/JSONL → tables (create/append).

## Vendored Driver

`vendor/rsfbclient-rust/` is a vendored copy of `rsfbclient-rust` 0.29.0 with a **NUMERIC/DECIMAL precision patch**. Upstream coerces scaled integers to `f64` (losing exactness — `NUMERIC(18,4)` is the standard money type in Firebird schemas). The patch keeps the scaled integer + scale in `xsqlda.rs` and renders the exact decimal as text at fetch time in `wire.rs`. Applied via `[patch.crates-io]` in the workspace `Cargo.toml`.

The vendor crate is a standalone workspace (its own `[workspace]` in `Cargo.toml`) so its unit tests can run independently:

```bash
cd vendor/rsfbclient-rust && cargo test --lib
```
