//! Code generation for development entities/structs from Firebird table schemas.
//!
//! This module is the dispatcher: it owns the [`Language`] enum, [`GenOptions`],
//! and the public [`generate`] entry point. The actual per-language logic lives
//! in [`targets`](super::targets), where each target implements the
//! [`Target`](super::targets::Target) trait with an exhaustive type mapping
//! over [`FbType`](super::FbType).

use super::targets::{
    GoTarget, JavaTarget, OpenApiTarget, ProtobufTarget, RustTarget, Target, TypeScriptTarget,
};
use crate::schema::{ForeignKeyInfo, TableSchema};

/// Language target for code generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    Java,
    Go,
    Rust,
    TypeScript,
    Protobuf,
    OpenApi,
    Lazarus,
    Svelte,
}

impl Language {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            "rust" => Some(Self::Rust),
            "typescript" | "ts" => Some(Self::TypeScript),
            "protobuf" | "proto" => Some(Self::Protobuf),
            "openapi" | "open_api" | "open-api" => Some(Self::OpenApi),
            "lazarus" | "delphi" => Some(Self::Lazarus),
            "svelte" => Some(Self::Svelte),
            _ => None,
        }
    }
}

/// Options for code generation.
#[derive(Debug, Clone)]
pub struct GenOptions {
    pub language: Language,
    pub include_fks: bool,
}

/// Generates code for one or more tables.
pub fn generate(schemas: &[TableSchema], fks: &[ForeignKeyInfo], opts: &GenOptions) -> String {
    let mut out = String::new();
    for schema in schemas {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        let table_fks: Vec<&ForeignKeyInfo> = fks
            .iter()
            .filter(|fk| fk.table.eq_ignore_ascii_case(&schema.table))
            .collect();
        match opts.language {
            Language::Java => {
                out.push_str(&JavaTarget.generate(schema, &table_fks, opts.include_fks))
            }
            Language::Go => out.push_str(&GoTarget.generate(schema, &table_fks, opts.include_fks)),
            Language::Rust => {
                out.push_str(&RustTarget.generate(schema, &table_fks, opts.include_fks))
            }
            Language::TypeScript => {
                out.push_str(&TypeScriptTarget.generate(schema, &table_fks, opts.include_fks))
            }
            Language::Protobuf => {
                out.push_str(&ProtobufTarget.generate(schema, &table_fks, opts.include_fks))
            }
            Language::OpenApi => {
                out.push_str(&OpenApiTarget.generate(schema, &table_fks, opts.include_fks))
            }
            Language::Lazarus => {
                // Lazarus is handled separately in the tool handler (generates .lfm files)
                out.push_str(&format!(
                    "// Lazarus form for table {} (use firebird_generate_dev with language=lazarus)\n",
                    schema.table
                ));
            }
            Language::Svelte => {
                // Svelte is handled separately in the tool handler (generates .svelte files)
                out.push_str(&format!(
                    "// Svelte page for table {} (use firebird_generate_dev with language=svelte)\n",
                    schema.table
                ));
            }
        }
    }
    out
}
