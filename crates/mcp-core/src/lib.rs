//! # firebird-5-mcp-core
//!
//! Firebird-specific database access and business logic.
//!
//! This crate builds on the shared MCP platform (`mcp-core` from mcp-rs)
//! and adds Firebird-specific features:
//!
//! - [`codegen`] — code generation for development artifacts
//! - [`compression`] — compression codecs for file export
//! - [`ddl`] — triggers, procedures, functions, sequences, drops
//! - [`explain`] — execution plans without running the query
//! - [`metadata`] — full DDL extraction (replaces `isql -x`)
//! - [`schema`] — schema introspection (tables, columns, indexes, FKs)
//! - [`stats`] — database statistics and encryption state (MON$ tables)
//! - [`storage`] — file storage for generated artifacts
//! - [`types`] — Firebird type formatting and identifier validation
//!
//! The shared platform types (`Config`, `FirebirdClient`, `Value`,
//! `QueryResult`, `mlog!`, `paths`) come from `mcp-core` and `mcp-types`.

/// Code generation for development artifacts from Firebird schemas.
pub mod codegen;
/// Compression codecs for file export (Snappy, ZSTD, LZ4, GZIP).
pub mod compression;
/// Triggers, procedures, functions, sequences, drops (no isql).
pub mod ddl;
/// Execution plans without running the query.
pub mod explain;
/// Full DDL extraction (replaces `isql -x`).
pub mod metadata;
/// Schema introspection (tables, columns, indexes, FKs).
pub mod schema;
/// Database statistics and encryption state (MON$ tables).
pub mod stats;
/// File storage for generated artifacts.
pub mod storage;
/// Firebird type formatting and identifier validation.
pub mod types;

// Re-export shared platform types for convenience.
pub use mcp_core::{
    Config, DatabaseConfig, FirebirdClient, IsolationPreset, ServerConfig, mlog, paths, query,
};
pub use mcp_types::{ColumnMeta, QueryResult, Value, format_cell, value_to_json};
