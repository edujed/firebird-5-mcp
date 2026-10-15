//! Handler for the `firebird_generate_view_dto` tool.

use firebird_5_mcp_core::config::Config;
use firebird_5_mcp_core::generator::Language;
use firebird_5_mcp_core::view_dto;
use serde_json::Value;

use super::common::{arg_str, open_client};

/// Handles the `firebird_generate_view_dto` tool call.
pub fn handle_generate_view_dto(cfg: &Config, args: &Value) -> Result<String, String> {
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
            "The 'language' argument is required (java, go, rust, typescript, protobuf, openapi)."
                .to_string()
        })?;
    let language = Language::parse(&language_str).ok_or_else(|| {
        format!(
            "Unsupported language: {language_str}. Use: java, go, rust, typescript, protobuf, openapi."
        )
    })?;

    let mut client = open_client(cfg, args)?;

    let code = view_dto::generate_view_dto(&mut client, &views, language)
        .map_err(|e| format!("Failed to generate view DTOs: {e}"))?;

    Ok(code)
}
