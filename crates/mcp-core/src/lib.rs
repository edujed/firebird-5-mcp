//! # mcp-core
//!
//! Firebird database access library (pure-Rust driver, no isql dependency).
//!
//! This crate is the reusable core of the `jed-personal-mcp` project. It
//! provides:
//!
//! - [`config`] — loading of `config.json` (same format as the Go version)
//! - [`client`] — a connection wrapper per database
//! - [`query`] — SELECT results, DML/DDL, scripts, batch inserts
//! - [`schema`] — schema introspection (tables, columns, indexes, FKs)
//! - [`ddl`] — triggers, procedures, functions, sequences, drops (no isql)
//! - [`explain`] — execution plans without running the query
//! - [`metadata`] — full DDL extraction (replaces `isql -x`)
//! - [`types`] — Firebird type formatting and identifier validation
//!
//! The MCP servers (`mcp-stdio-bin` for stdio, and the future `mcp-http-bin`
//! for HTTP/SSE) sit on top of this library and expose the same tool set.

pub mod client;
pub mod codegen;
pub mod compression;
pub mod config;
pub mod ddl;
pub mod explain;
pub mod log;
pub mod metadata;
pub mod paths;
pub mod query;
pub mod schema;
pub mod stats;
pub mod storage;
pub mod types;

pub use client::{FirebirdClient, IsolationPreset};
pub use config::{Config, DatabaseConfig, ServerConfig};
