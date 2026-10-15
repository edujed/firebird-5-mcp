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
//! The MCP server (`mcp-bin`) sits on top of this library and exposes the
//! same tool set as the Go implementation.

pub mod client;
pub mod config;
pub mod ddl;
pub mod explain;
pub mod generator;
pub mod lazarus;
pub mod metadata;
pub mod query;
pub mod schema;
pub mod svelte;
pub mod types;
pub mod view_dto;

pub use client::FirebirdClient;
pub use config::{Config, DatabaseConfig, ServerConfig};
