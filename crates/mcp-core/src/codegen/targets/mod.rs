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
pub use rust::RustTarget;
pub use typescript::TypeScriptTarget;

/// A code-generation target for a single language.
///
/// `type_for` is the exhaustive Firebird→language type mapping; `generate`
/// emits the full entity (class/struct/interface/message/schema) for a table.
pub trait Target {
    /// Maps a Firebird type to this language's type name.
    fn type_for(&self, t: &FbType) -> String;

    /// Generates the entity for one table.
    fn generate(&self, schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String;

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
