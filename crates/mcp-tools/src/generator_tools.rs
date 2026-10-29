//! Handler for the `firebird_generate_dev` tool.

use firebird_5_mcp_core::codegen::targets::GeneratedFile;
use firebird_5_mcp_core::codegen::ui::{lazarus, svelte};
use firebird_5_mcp_core::codegen::{GenOptions, Language, generator};
use firebird_5_mcp_core::schema;
use mcp_core::Config;
use mcp_types::ToolResult;
use serde_json::Value;

use super::save::save_or_inline;
use super::{arg_bool, arg_str, open_client};

/// Handles the `firebird_generate_dev` tool call.
pub fn handle_generate_dev(
    cfg: &Config,
    args: &Value,
    files_base_url: Option<&str>,
) -> Result<ToolResult, String> {
    let tables_raw = args
        .get("tables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "The 'tables' argument is required and must be an array of strings.".to_string()
        })?;

    let tables: Vec<String> = tables_raw
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_uppercase())
        .collect();

    if tables.is_empty() {
        return Err("The 'tables' array must contain at least one table name.".to_string());
    }
    if tables.len() > 20 {
        return Err(format!(
            "Maximum 20 tables per request (got {}).",
            tables.len()
        ));
    }

    let language_str = arg_str(args, "language")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "The 'language' argument is required (java, go, rust, typescript, protobuf, openapi, python, lazarus, svelte)."
                .to_string()
        })?;
    let language = Language::parse(&language_str).ok_or_else(|| {
        format!(
            "Unsupported language: {language_str}. Use: java, go, rust, typescript, protobuf, openapi, graphql, python, lazarus, svelte."
        )
    })?;

    let include_fks = arg_bool(args, "include_fks").unwrap_or(false);
    let include_comments = arg_bool(args, "include_comments").unwrap_or(true);
    let include_examples = arg_bool(args, "include_examples").unwrap_or(false);
    let package = arg_str(args, "package").filter(|s| !s.is_empty());

    let mut client = open_client(cfg, args)?;

    // Fetch schemas for all tables
    let mut schemas = Vec::new();
    for table in &tables {
        let schema = schema::describe_table(&mut client, table)
            .map_err(|e| format!("Failed to describe table {table}: {e}"))?;
        schemas.push(schema);
    }

    // Fetch all FKs (filtered per-table in the generator)
    let fks = schema::get_foreign_keys(&mut client)
        .map_err(|e| format!("Failed to fetch foreign keys: {e}"))?;

    let opts = GenOptions {
        language,
        include_fks,
        include_comments,
        include_examples,
        package,
        is_view: false,
    };

    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
    let comp_suffix = compression_suffix(args)?;

    // Special case: Lazarus generates .lfm files, not code entities
    if language == Language::Lazarus {
        let mut files = Vec::new();
        for schema in &schemas {
            let lazarus_fields: Vec<lazarus::LazarusField> = schema
                .columns
                .iter()
                .map(lazarus::from_column_info)
                .collect();
            files.push(GeneratedFile::new(
                format!("{}.lfm{}", schema.table.to_lowercase(), comp_suffix),
                lazarus::generate_lfm(&schema.table, &lazarus_fields, false),
                "lazarus",
            ));
        }
        return save_or_inline(cfg, args, &db_name, "lazarus", &files, files_base_url);
    }

    // Special case: Svelte generates .svelte files (Page + Form)
    if language == Language::Svelte {
        let mut files = Vec::new();
        for schema in &schemas {
            // Find PK: look for a unique index named PK_* in schema.indexes
            let pk_field = schema
                .indexes
                .iter()
                .find(|idx| idx.unique && idx.name.to_uppercase().starts_with("PK_"))
                .and_then(|idx| {
                    idx.columns
                        .split(',')
                        .next()
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                })
                .unwrap_or_else(|| {
                    schema
                        .columns
                        .first()
                        .map(|c| c.name.clone())
                        .unwrap_or_default()
                });

            // Find FKs
            let table_fks: Vec<&schema::ForeignKeyInfo> = fks
                .iter()
                .filter(|fk| fk.table.eq_ignore_ascii_case(&schema.table))
                .collect();
            let fk_fields: Vec<String> = table_fks.iter().map(|fk| fk.column.clone()).collect();

            // Create Svelte fields
            let svelte_fields: Vec<svelte::SvelteField> = schema
                .columns
                .iter()
                .map(|col| {
                    let is_pk = col.name == pk_field;
                    let is_fk = fk_fields.contains(&col.name);
                    svelte::from_column_info(col, is_pk, is_fk)
                })
                .collect();

            // Generate Page component
            let page = format!("// Page component for {}\n", schema.table)
                + &svelte::generate_page(&schema.table, &svelte_fields, &pk_field, &fk_fields);
            files.push(GeneratedFile::new(
                format!("{}_page.svelte{}", schema.table.to_lowercase(), comp_suffix),
                page,
                "svelte",
            ));

            // Generate Form component
            let form = format!("// Form component for {}\n", schema.table)
                + &svelte::generate_form(&schema.table, &svelte_fields);
            files.push(GeneratedFile::new(
                format!("{}_form.svelte{}", schema.table.to_lowercase(), comp_suffix),
                form,
                "svelte",
            ));
        }
        return save_or_inline(cfg, args, &db_name, "svelte", &files, files_base_url);
    }

    let mut files = generator::generate(&schemas, &fks, &opts);
    if !comp_suffix.is_empty() {
        for f in &mut files {
            f.name.push_str(&comp_suffix);
        }
    }
    let format = match language {
        Language::Java => "java",
        Language::Go => "go",
        Language::Rust => "rust",
        Language::TypeScript => "typescript",
        Language::Protobuf => "protobuf",
        Language::OpenApi => "openapi",
        Language::Graphql => "graphql",
        Language::Python => "python",
        Language::JsonSchema => "json-schema",
        _ => "unknown",
    };
    save_or_inline(cfg, args, &db_name, format, &files, files_base_url)
}

/// Reads the optional `compression` argument and returns the file suffix to
/// append (e.g. `.zst`). Returns an empty string when compression is `none`
/// or omitted.
pub fn compression_suffix(args: &Value) -> Result<String, String> {
    let comp = arg_str(args, "compression").filter(|s| !s.is_empty());
    match comp.as_deref() {
        None | Some("none") => Ok(String::new()),
        Some("zstd") => Ok(".zst".to_string()),
        Some("gzip") => Ok(".gz".to_string()),
        Some("lz4") => Ok(".lz4".to_string()),
        Some(other) => Err(format!(
            "unsupported compression '{other}' — valid: zstd, gzip, lz4, none"
        )),
    }
}
