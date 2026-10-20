//! Handler for the `firebird_generate_view_dto` tool.

use firebird_5_mcp_core::codegen::view_dto;
use firebird_5_mcp_core::codegen::{GenOptions, Language};
use firebird_5_mcp_core::config::Config;
use serde_json::Value;

use super::ToolResult;
use super::common::{arg_bool, arg_str, open_client, save_or_inline};

/// Handles the `firebird_generate_view_dto` tool call.
pub fn handle_generate_view_dto(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let views_raw = args
        .get("views")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "The 'views' argument is required and must be an array of strings.".to_string()
        })?;

    let views: Vec<String> = views_raw
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_uppercase())
        .collect();

    if views.is_empty() {
        return Err("The 'views' array must contain at least one view name.".to_string());
    }
    if views.len() > 20 {
        return Err(format!(
            "Maximum 20 views per request (got {}).",
            views.len()
        ));
    }

    let language_str = arg_str(args, "language")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "The 'language' argument is required (java, go, rust, typescript, protobuf, openapi, python)."
                .to_string()
        })?;
    let language = Language::parse(&language_str).ok_or_else(|| {
        format!(
            "Unsupported language: {language_str}. Use: java, go, rust, typescript, protobuf, openapi, graphql, python."
        )
    })?;

    let include_comments = arg_bool(args, "include_comments").unwrap_or(true);
    let include_examples = arg_bool(args, "include_examples").unwrap_or(false);
    let package = arg_str(args, "package").filter(|s| !s.is_empty());

    let mut client = open_client(cfg, args)?;

    let opts = GenOptions {
        language,
        include_fks: false,
        include_comments,
        include_examples,
        package,
        is_view: true,
    };

    let files = view_dto::generate_view_dto(&mut client, &views, &opts)
        .map_err(|e| format!("Failed to generate view DTOs: {e}"))?;

    let db_name = arg_str(args, "database").unwrap_or_else(|| cfg.default_database().to_string());
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
    save_or_inline(cfg, args, &db_name, format, &files)
}
