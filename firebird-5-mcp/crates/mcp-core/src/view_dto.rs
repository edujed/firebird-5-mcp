//! View DTO generation: generates DTOs/records for database views.
//!
//! Unlike table generation (which uses RDB$RELATION_FIELDS), view DTOs use
//! resolved metadata from executing `SELECT FIRST 1 * FROM view` to get the
//! actual column types after all joins and calculations.

use rsfbclient::FbError;

use crate::client::FirebirdClient;
use crate::generator::{Language, to_camel_case, to_pascal_case, to_snake_case};
use crate::lazarus;
use crate::query::{Value, query};
use crate::svelte;

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
        // Infer type from the actual value in the first row (if any)
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
        // Get resolved metadata
        let fields = get_view_metadata(client, view)?;

        // Generate code based on language
        match language {
            Language::Java => out.push_str(&generate_java_dto(view, &fields)),
            Language::Go => out.push_str(&generate_go_dto(view, &fields)),
            Language::Rust => out.push_str(&generate_rust_dto(view, &fields)),
            Language::TypeScript => out.push_str(&generate_ts_dto(view, &fields)),
            Language::Protobuf => out.push_str(&generate_protobuf_dto(view, &fields)),
            Language::OpenApi => out.push_str(&generate_openapi_dto(view, &fields)),
            Language::Lazarus => {
                let lazarus_fields: Vec<lazarus::LazarusField> =
                    fields.iter().map(lazarus::from_view_field).collect();
                out.push_str(&lazarus::generate_lfm(view, &lazarus_fields, true));
            }
            Language::Svelte => {
                let svelte_fields: Vec<svelte::SvelteField> =
                    fields.iter().map(svelte::from_view_field).collect();
                out.push_str(&format!(
                    "// Page component for view {} (read-only)\n",
                    view
                ));
                out.push_str(&svelte::generate_page_for_view(view, &svelte_fields));
            }
        }
    }

    Ok(out)
}

// --- Java DTO ---

fn generate_java_dto(view: &str, fields: &[ViewField]) -> String {
    let class_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("import lombok.Data;\n\n");
    out.push_str("@Data\n");
    out.push_str(&format!("public class {} {{\n", class_name));

    for field in fields {
        let java_type = fb_type_to_java(&field.type_str);
        let field_name = to_camel_case(&field.name);
        out.push_str(&format!("    private {} {};\n", java_type, field_name));
    }

    out.push_str("}\n\n");
    out
}

fn fb_type_to_java(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "String".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "Long".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "java.math.BigDecimal".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "Boolean".to_string()
    } else if fb_type.starts_with("TIMESTAMP") {
        "java.time.LocalDateTime".to_string()
    } else if fb_type.starts_with("DATE") {
        "java.time.LocalDate".to_string()
    } else if fb_type.starts_with("TIME") {
        "java.time.LocalTime".to_string()
    } else if fb_type.starts_with("BLOB") {
        "byte[]".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "Double".to_string()
    } else {
        "Object".to_string()
    }
}

// --- Go DTO ---

fn generate_go_dto(view: &str, fields: &[ViewField]) -> String {
    let struct_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str(&format!("type {} struct {{\n", struct_name));

    for field in fields {
        let go_type = fb_type_to_go(&field.type_str);
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

fn fb_type_to_go(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "sql.NullString".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "sql.NullInt64".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "sql.NullFloat64".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "sql.NullBool".to_string()
    } else if fb_type.starts_with("TIMESTAMP") {
        "sql.NullTime".to_string()
    } else if fb_type.starts_with("DATE") {
        "sql.NullTime".to_string()
    } else if fb_type.starts_with("TIME") {
        "sql.NullTime".to_string()
    } else if fb_type.starts_with("BLOB") {
        "[]byte".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "sql.NullFloat64".to_string()
    } else {
        "interface{}".to_string()
    }
}

// --- Rust DTO ---

fn generate_rust_dto(view: &str, fields: &[ViewField]) -> String {
    let struct_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("#[derive(Debug, FromRow)]\n");
    out.push_str(&format!("pub struct {} {{\n", struct_name));

    for field in fields {
        let rust_type = fb_type_to_rust(&field.type_str);
        let field_name = to_snake_case(&field.name);
        out.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
    }

    out.push_str("}\n\n");
    out
}

fn fb_type_to_rust(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "Option<String>".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "Option<i64>".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "Option<f64>".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "Option<bool>".to_string()
    } else if fb_type.starts_with("TIMESTAMP") {
        "Option<NaiveDateTime>".to_string()
    } else if fb_type.starts_with("DATE") {
        "Option<NaiveDate>".to_string()
    } else if fb_type.starts_with("TIME") {
        "Option<NaiveTime>".to_string()
    } else if fb_type.starts_with("BLOB") {
        "Option<Vec<u8>>".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "Option<f64>".to_string()
    } else {
        "Option<String>".to_string()
    }
}

// --- TypeScript DTO ---

fn generate_ts_dto(view: &str, fields: &[ViewField]) -> String {
    let interface_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str(&format!("export interface {} {{\n", interface_name));

    for field in fields {
        let ts_type = fb_type_to_ts(&field.type_str);
        let field_name = to_camel_case(&field.name);
        let optional = if field.nullable { "?" } else { "" };
        out.push_str(&format!("    {}{}: {};\n", field_name, optional, ts_type));
    }

    out.push_str("}\n\n");
    out
}

fn fb_type_to_ts(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "string".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "number".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "number".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "boolean".to_string()
    } else if fb_type.starts_with("TIMESTAMP")
        || fb_type.starts_with("DATE")
        || fb_type.starts_with("TIME")
    {
        "string".to_string()
    } else if fb_type.starts_with("BLOB") {
        "Uint8Array".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "number".to_string()
    } else {
        "any".to_string()
    }
}

// --- Protobuf DTO ---

fn generate_protobuf_dto(view: &str, fields: &[ViewField]) -> String {
    let message_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("// DTO gerado da view {} (read-only)\n", view));
    out.push_str("syntax = \"proto3\";\n\n");
    out.push_str(&format!("message {} {{\n", message_name));

    for (i, field) in fields.iter().enumerate() {
        let proto_type = fb_type_to_protobuf(&field.type_str);
        let field_name = to_snake_case(&field.name);
        out.push_str(&format!("    {} {} = {};\n", proto_type, field_name, i + 1));
    }

    out.push_str("}\n\n");
    out
}

fn fb_type_to_protobuf(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "string".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "int64".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "double".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "bool".to_string()
    } else if fb_type.starts_with("TIMESTAMP")
        || fb_type.starts_with("DATE")
        || fb_type.starts_with("TIME")
    {
        "string".to_string()
    } else if fb_type.starts_with("BLOB") {
        "bytes".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "double".to_string()
    } else {
        "string".to_string()
    }
}

// --- OpenAPI DTO ---

fn generate_openapi_dto(view: &str, fields: &[ViewField]) -> String {
    let _schema_name = to_pascal_case(view);
    let mut out = String::new();

    out.push_str(&format!("# Schema gerado da view {} (read-only)\n", view));
    out.push_str("type: object\n");
    out.push_str("properties:\n");

    for field in fields {
        let openapi_type = fb_type_to_openapi(&field.type_str);
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

fn fb_type_to_openapi(fb_type: &str) -> String {
    if fb_type.starts_with("VARCHAR")
        || fb_type.starts_with("CHAR")
        || fb_type.starts_with("NVARCHAR")
    {
        "string".to_string()
    } else if fb_type.starts_with("INTEGER")
        || fb_type.starts_with("SMALLINT")
        || fb_type.starts_with("BIGINT")
    {
        "integer".to_string()
    } else if fb_type.starts_with("NUMERIC")
        || fb_type.starts_with("DECIMAL")
        || fb_type.starts_with("DECFLOAT")
    {
        "number".to_string()
    } else if fb_type.starts_with("BOOLEAN") {
        "boolean".to_string()
    } else if fb_type.starts_with("TIMESTAMP")
        || fb_type.starts_with("DATE")
        || fb_type.starts_with("TIME")
    {
        "string".to_string()
    } else if fb_type.starts_with("BLOB") {
        "string".to_string()
    } else if fb_type.starts_with("FLOAT") || fb_type.starts_with("DOUBLE") {
        "number".to_string()
    } else {
        "string".to_string()
    }
}
