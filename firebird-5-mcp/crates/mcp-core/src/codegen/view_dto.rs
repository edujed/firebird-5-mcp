//! View DTO generation: generates DTOs/records for database views.
//!
//! Unlike table generation (which uses RDB$RELATION_FIELDS), view DTOs use
//! resolved metadata from executing `SELECT FIRST 1 * FROM view` to get the
//! actual column types after all joins and calculations.
//!
//! Type mapping is delegated to each target's [`dto_type_for`](super::targets::Target::dto_type_for),
//! so there is no duplicated Firebird→language mapping in this file.

use rsfbclient::FbError;

use super::fbtype::FbType;
use super::generator::Language;
use super::naming::{to_camel_case, to_pascal_case, to_snake_case};
use super::targets::{
    GoTarget, JavaTarget, OpenApiTarget, ProtobufTarget, RustTarget, Target, TypeScriptTarget,
};
use super::ui::{lazarus, svelte};
use crate::client::FirebirdClient;
use crate::query::{Value, query};

/// A field in a generated DTO.
#[derive(Debug, Clone)]
pub struct ViewField {
    pub name: String,
    pub type_str: String,
    pub nullable: bool,
}

/// Gets the resolved metadata for a view by executing `SELECT FIRST 1 * FROM view`.
/// Returns the column names and types as resolved by the Firebird engine.
///
/// Types are inferred from the actual `Value` in the first row (more robust
/// than relying on driver raw_type codes, which are not RDB$FIELD_TYPE codes).
pub fn get_view_metadata(
    client: &mut FirebirdClient,
    view_name: &str,
) -> Result<Vec<ViewField>, FbError> {
    let sql = format!("SELECT FIRST 1 * FROM {}", view_name);
    let result = query(client, &sql)?;

    let mut fields = Vec::new();
    for (i, col) in result.columns.iter().enumerate() {
        let type_str = infer_type_from_value(result.get(0, i));
        fields.push(ViewField {
            name: col.name.clone(),
            type_str,
            nullable: true, // Views are read-only, assume nullable for safety
        });
    }

    Ok(fields)
}

/// Infers a Firebird type string from an actual `Value` in the result row.
/// This is more robust than relying on driver raw_type codes, which are
/// internal driver codes (e.g., 580, 448, 32764, 510) and not RDB$FIELD_TYPE.
fn infer_type_from_value(v: &Value) -> String {
    match v {
        Value::Null => "VARCHAR".to_string(), // Fallback: assume text
        Value::Bool(_) => "BOOLEAN".to_string(),
        Value::Int(_) => "INTEGER".to_string(),
        Value::Float(_) => "DOUBLE PRECISION".to_string(),
        Value::Text(_) => "VARCHAR".to_string(),
        Value::Date(_) => "DATE".to_string(),
        Value::Time(_) => "TIME".to_string(),
        Value::Timestamp(_) => "TIMESTAMP".to_string(),
        Value::Binary(_) => "BLOB".to_string(),
    }
}

/// Generates DTO code for a view in the specified language.
pub fn generate_view_dto(
    client: &mut FirebirdClient,
    views: &[String],
    language: Language,
) -> Result<String, FbError> {
    let mut out = String::new();

    for view in views {
        let fields = get_view_metadata(client, view)?;
        out.push_str(&generate_single_view_dto(view, &fields, language));
    }

    Ok(out)
}

/// Generates the DTO for a single view, dispatching to the appropriate target.
fn generate_single_view_dto(view: &str, fields: &[ViewField], language: Language) -> String {
    match language {
        Language::Java => gen_java_dto(view, fields, &JavaTarget),
        Language::Go => gen_go_dto(view, fields, &GoTarget),
        Language::Rust => gen_rust_dto(view, fields, &RustTarget),
        Language::TypeScript => gen_ts_dto(view, fields, &TypeScriptTarget),
        Language::Protobuf => gen_protobuf_dto(view, fields, &ProtobufTarget),
        Language::OpenApi => gen_openapi_dto(view, fields, &OpenApiTarget),
        Language::Lazarus => {
            let lazarus_fields: Vec<lazarus::LazarusField> =
                fields.iter().map(lazarus::from_view_field).collect();
            lazarus::generate_lfm(view, &lazarus_fields, true)
        }
        Language::Svelte => {
            let svelte_fields: Vec<svelte::SvelteField> =
                fields.iter().map(svelte::from_view_field).collect();
            format!("// Page component for view {} (read-only)\n", view)
                + &svelte::generate_page_for_view(view, &svelte_fields)
        }
    }
}

// --- Java DTO ---

fn gen_java_dto(view: &str, fields: &[ViewField], target: &JavaTarget) -> String {
    let class_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("import lombok.Data;\n\n");
    out.push_str("@Data\n");
    out.push_str(&format!("public class {} {{\n", class_name));

    for field in fields {
        let fb = FbType::from_type_str(&field.type_str);
        let java_type = target.dto_type_for(&fb);
        let field_name = to_camel_case(&field.name);
        out.push_str(&format!("    private {} {};\n", java_type, field_name));
    }

    out.push_str("}\n\n");
    out
}

// --- Go DTO ---

fn gen_go_dto(view: &str, fields: &[ViewField], target: &GoTarget) -> String {
    let struct_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str(&format!("type {} struct {{\n", struct_name));

    for field in fields {
        let fb = FbType::from_type_str(&field.type_str);
        let go_type = target.dto_type_for(&fb);
        let field_name = to_pascal_case(&field.name);
        let db_tag = to_snake_case(&field.name);
        let json_tag = to_camel_case(&field.name);
        out.push_str(&format!(
            "    {} {} `db:\"{}\" json:\"{}\"`\n",
            field_name, go_type, db_tag, json_tag
        ));
    }

    out.push_str("}\n\n");
    out
}

// --- Rust DTO ---

fn gen_rust_dto(view: &str, fields: &[ViewField], target: &RustTarget) -> String {
    let struct_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("#[derive(Debug, FromRow)]\n");
    out.push_str(&format!("pub struct {} {{\n", struct_name));

    for field in fields {
        let fb = FbType::from_type_str(&field.type_str);
        let rust_type = target.dto_type_for(&fb);
        let field_name = to_snake_case(&field.name);
        out.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
    }

    out.push_str("}\n\n");
    out
}

// --- TypeScript DTO ---

fn gen_ts_dto(view: &str, fields: &[ViewField], target: &TypeScriptTarget) -> String {
    let interface_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str(&format!("export interface {} {{\n", interface_name));

    for field in fields {
        let fb = FbType::from_type_str(&field.type_str);
        let ts_type = target.dto_type_for(&fb);
        let field_name = to_camel_case(&field.name);
        let optional = if field.nullable { "?" } else { "" };
        out.push_str(&format!("    {}{}: {};\n", field_name, optional, ts_type));
    }

    out.push_str("}\n\n");
    out
}

// --- Protobuf DTO ---

fn gen_protobuf_dto(view: &str, fields: &[ViewField], target: &ProtobufTarget) -> String {
    let message_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("syntax = \"proto3\";\n\n");
    out.push_str(&format!("message {} {{\n", message_name));

    for (i, field) in fields.iter().enumerate() {
        let fb = FbType::from_type_str(&field.type_str);
        let proto_type = target.dto_type_for(&fb);
        let field_name = to_snake_case(&field.name);
        out.push_str(&format!("    {} {} = {};\n", proto_type, field_name, i + 1));
    }

    out.push_str("}\n\n");
    out
}

// --- OpenAPI DTO ---

fn gen_openapi_dto(view: &str, fields: &[ViewField], target: &OpenApiTarget) -> String {
    let mut out = String::new();

    out.push_str(&format!("# Schema gerado da view {} (read-only)\n", view));
    out.push_str("type: object\n");
    out.push_str("properties:\n");

    for field in fields {
        let fb = FbType::from_type_str(&field.type_str);
        let openapi_type = target.dto_type_for(&fb);
        let field_name = to_camel_case(&field.name);
        out.push_str(&format!("  {}:\n", field_name));
        out.push_str(&format!("    type: {}\n", openapi_type));
        if field.nullable {
            out.push_str("    nullable: true\n");
        }
    }

    out.push('\n');
    out
}
