//! Code-generation targets, one module per language.
//!
//! Each target implements the [`Target`] trait: an exhaustive type mapping
//! over [`FbType`](super::FbType) plus the entity generator. Adding a new
//! language means adding a new module here and one arm in
//! [`generator::generate`](super::generator::generate) — no other target is
//! touched.

mod go;
mod graphql;
mod java;
mod openapi;
mod protobuf;
mod python;
mod rust;
mod typescript;

use super::fbtype::FbType;
use super::generator::GenOptions;
use crate::schema::{ForeignKeyInfo, TableSchema};

pub use go::GoTarget;
pub use graphql::GraphqlTarget;
pub use java::JavaTarget;
pub use openapi::OpenApiTarget;
pub use protobuf::ProtobufTarget;
pub use python::PythonTarget;
pub use rust::RustTarget;
pub use typescript::TypeScriptTarget;

/// A single generated file.
///
/// `name` is the file name (e.g. `"User.java"`, `"schema.graphql"`),
/// `content` is the full file content, and `format` identifies the output
/// format (e.g. `"java"`, `"graphql-schema"`, `"mermaid"`). The `format`
/// field is used to determine the file extension and to label the file in
/// structured output.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub name: String,
    pub content: String,
    pub format: String,
}

impl GeneratedFile {
    /// Creates a new generated file.
    pub fn new(
        name: impl Into<String>,
        content: impl Into<String>,
        format: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            format: format.into(),
        }
    }
}

/// A code-generation target for a single language.
///
/// `type_for` is the exhaustive Firebird→language type mapping; `generate`
/// emits the full entity (class/struct/interface/message/schema) for a table.
pub trait Target {
    /// Maps a Firebird type to this language's type name.
    fn type_for(&self, t: &FbType) -> String;

    /// Generates the entity for one table.
    ///
    /// Most targets return a single file. Targets that produce multiple
    /// artifacts (e.g. GraphQL: schema + resolvers) return one
    /// [`GeneratedFile`] per artifact.
    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile>;

    /// Maps a Firebird type to this language's *DTO/record* type name.
    ///
    /// DTOs (view records, API payloads) typically wrap every field in a
    /// nullable type (`Option<T>`, `sql.Null*`, …) because view columns are
    /// always nullable. The default delegates to [`type_for`], which is
    /// correct for languages that have no distinct null-wrapper convention
    /// (Java, TypeScript, Protobuf, OpenAPI).
    fn dto_type_for(&self, t: &FbType) -> String {
        self.type_for(t)
    }
}
