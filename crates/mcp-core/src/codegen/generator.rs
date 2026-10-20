//! Code generation for development entities/structs from Firebird table schemas.
//!
//! This module is the dispatcher: it owns the [`Language`] enum, [`GenOptions`],
//! and the public [`generate`] entry point. The actual per-language logic lives
//! in [`targets`](super::targets), where each target implements the
//! [`Target`](super::targets::Target) trait with an exhaustive type mapping
//! over [`FbType`](super::FbType).

use super::targets::{
    GeneratedFile, GoTarget, GraphqlTarget, JavaTarget, JsonSchemaTarget, OpenApiTarget,
    ProtobufTarget, PythonTarget, RustTarget, Target, TypeScriptTarget,
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
    Python,
    JsonSchema,
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
            "python" | "py" => Some(Self::Python),
            "json_schema" | "jsonschema" | "json-schema" => Some(Self::JsonSchema),
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
///
/// Returns a list of [`GeneratedFile`]s. Each table produces one or more
/// files depending on the target (most targets produce one file per table;
/// GraphQL produces two: schema + resolvers).
pub fn generate(
    schemas: &[TableSchema],
    fks: &[ForeignKeyInfo],
    opts: &GenOptions,
) -> Vec<GeneratedFile> {
    let mut files = Vec::new();
    for schema in schemas {
        let table_fks: Vec<&ForeignKeyInfo> = fks
            .iter()
            .filter(|fk| fk.table.eq_ignore_ascii_case(&schema.table))
            .collect();
        let generated = match opts.language {
            Language::Java => JavaTarget.generate(schema, &table_fks, opts),
            Language::Go => GoTarget.generate(schema, &table_fks, opts),
            Language::Rust => RustTarget.generate(schema, &table_fks, opts),
            Language::TypeScript => TypeScriptTarget.generate(schema, &table_fks, opts),
            Language::Protobuf => ProtobufTarget.generate(schema, &table_fks, opts),
            Language::OpenApi => OpenApiTarget.generate(schema, &table_fks, opts),
            Language::Graphql => GraphqlTarget.generate(schema, &table_fks, opts),
            Language::Python => PythonTarget.generate(schema, &table_fks, opts),
            Language::JsonSchema => JsonSchemaTarget.generate(schema, &table_fks, opts),
            Language::Lazarus => {
                // Lazarus is handled separately in the tool handler (generates .lfm files)
                vec![GeneratedFile::new(
                    format!("{}.lfm", schema.table.to_lowercase()),
                    format!(
                        "// Lazarus form for table {} (use firebird_generate_dev with language=lazarus)\n",
                        schema.table
                    ),
                    "lazarus",
                )]
            }
            Language::Svelte => {
                // Svelte is handled separately in the tool handler (generates .svelte files)
                vec![GeneratedFile::new(
                    format!("{}.svelte", schema.table.to_lowercase()),
                    format!(
                        "// Svelte page for table {} (use firebird_generate_dev with language=svelte)\n",
                        schema.table
                    ),
                    "svelte",
                )]
            }
        };
        files.extend(generated);
    }
    files
}
