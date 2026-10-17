//! Code generation for development entities/structs from Firebird table schemas.
//!
//! This module is the dispatcher: it owns the [`Language`] enum, [`GenOptions`],
//! and the public [`generate`] entry point. The actual per-language logic lives
//! in [`targets`](super::targets), where each target implements the
//! [`Target`](super::targets::Target) trait with an exhaustive type mapping
//! over [`FbType`](super::FbType).

use super::targets::{
    GoTarget, GraphqlTarget, JavaTarget, OpenApiTarget, ProtobufTarget, RustTarget, Target,
    TypeScriptTarget,
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
    Graphql,
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
            "graphql" | "gql" => Some(Self::Graphql),
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
    /// Emit column comments (`/// Column: X`, `// FK → ...`). Default: true.
    pub include_comments: bool,
    /// Emit example usage (query + deserialization). Default: false.
    pub include_examples: bool,
    /// Package/namespace to emit at the top (Java: `com.app.entity`,
    /// Go: `models`, Protobuf: `app.domain`). Default: None.
    pub package: Option<String>,
    /// When true, suppress entity-specific annotations (Java: `@Entity`,
    /// `@Table`, `@Id`, `@GeneratedValue`). Used for view DTOs, which are
    /// read-only projections and should not be mapped as JPA entities.
    /// Default: false.
    pub is_view: bool,
}

impl GenOptions {
    /// Creates options with sensible defaults (comments on, examples off, no package).
    pub fn new(language: Language, include_fks: bool) -> Self {
        Self {
            language,
            include_fks,
            include_comments: true,
            include_examples: false,
            package: None,
            is_view: false,
        }
    }
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
            Language::Java => out.push_str(&JavaTarget.generate(schema, &table_fks, opts)),
            Language::Go => out.push_str(&GoTarget.generate(schema, &table_fks, opts)),
            Language::Rust => out.push_str(&RustTarget.generate(schema, &table_fks, opts)),
            Language::TypeScript => {
                out.push_str(&TypeScriptTarget.generate(schema, &table_fks, opts))
            }
            Language::Protobuf => out.push_str(&ProtobufTarget.generate(schema, &table_fks, opts)),
            Language::OpenApi => out.push_str(&OpenApiTarget.generate(schema, &table_fks, opts)),
            Language::Graphql => out.push_str(&GraphqlTarget.generate(schema, &table_fks, opts)),
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
