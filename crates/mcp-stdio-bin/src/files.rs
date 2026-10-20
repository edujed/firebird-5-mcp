//! File upload/download endpoints for the SSE transport.
//!
//! Provides:
//! - `GET /files?path=<rel>` — downloads a file from the storage directory
//!   (streaming, with path-traversal protection).
//! - `POST /files` — multipart upload of a file to the storage directory
//!   (500 MB limit).
//!
//! Both endpoints use the same bearer-token authentication as the rest of
//! the SSE server. The storage directory is resolved from the live config
//! (same priority as the tools: `config.storage.dir` > `~/.firebird-mcp/output/`).

use crate::auth::{AuthConfig, check_auth};
use axum::extract::multipart::Multipart;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_tools::ProtocolCore;
use serde::Deserialize;
use serde_json::json;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// Maximum upload size: 500 MB.
const MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;

/// Query parameters for `GET /files`.
#[derive(Deserialize)]
struct FileQuery {
    /// Relative path within the storage directory (e.g. `prod_db_csv/data.csv`).
    path: String,
}

/// Shared state for the file endpoints.
#[derive(Clone)]
struct FilesState {
    core: Arc<ProtocolCore>,
    auth: Arc<AuthConfig>,
}

/// Builds the axum router for the file endpoints.
pub fn build_files_router(core: Arc<ProtocolCore>, auth: Arc<AuthConfig>) -> Router {
    let state = FilesState { core, auth };
    Router::new()
        .route(
            "/files",
            get(
                |State(state): State<Arc<FilesState>>,
                 headers: HeaderMap,
                 query: Query<FileQuery>| {
                    handle_download(state, headers, query)
                },
            )
            .post(
                |State(state): State<Arc<FilesState>>, headers: HeaderMap, multipart: Multipart| {
                    handle_upload(state, headers, multipart)
                },
            )
            .options(cors_preflight),
        )
        .with_state(Arc::new(state))
}

/// Handles CORS preflight requests (OPTIONS).
///
/// Echoes the requested headers from `Access-Control-Request-Headers` so the
/// browser knows which headers are allowed.
async fn cors_preflight(headers: HeaderMap) -> Response {
    let requested_headers = headers
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Authorization, Content-Type");

    (
        StatusCode::NO_CONTENT,
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS"),
            (header::ACCESS_CONTROL_ALLOW_HEADERS, requested_headers),
            (header::ACCESS_CONTROL_MAX_AGE, "86400"),
        ],
    )
        .into_response()
}

/// Adds CORS headers to a response.
fn add_cors_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    response
}

/// Resolves the storage base directory from the live config.
///
/// Mirrors `mcp-tools::common::storage_dir` (without the per-call
/// `outputDirPath` override, which doesn't apply to HTTP endpoints).
fn resolve_storage_dir(core: &ProtocolCore) -> Option<PathBuf> {
    let guard = core.config();
    let cfg = guard.as_ref()?;
    if let Some(storage) = &cfg.storage {
        if let Some(dir) = storage.effective_dir() {
            return Some(dir);
        }
    }
    firebird_5_mcp_core::storage::default_storage_dir()
}

/// Resolves a relative path against the storage base directory, rejecting
/// any path that escapes the base (path traversal).
///
/// Returns the canonical absolute path on success.
fn resolve_safe_path(base: &Path, rel: &str) -> Result<PathBuf, Response> {
    // Reject empty or absolute paths.
    if rel.is_empty() || Path::new(rel).is_absolute() {
        return Err(bad_request("path must be a non-empty relative path"));
    }

    let candidate = base.join(rel);

    // Reject `..` components before canonicalize (the path may not exist yet
    // for uploads).
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(bad_request("path traversal not allowed"));
    }

    // Canonicalize to resolve symlinks and verify containment.
    let canonical_base = base
        .canonicalize()
        .map_err(|e| internal_error(format!("storage dir not accessible: {e}")))?;

    let canonical = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|e| not_found(format!("file not found: {e}")))?
    } else {
        // For uploads the file may not exist yet — canonicalize the deepest
        // existing ancestor and re-join the remainder.
        let mut existing = candidate.clone();
        let mut remainder: Vec<String> = Vec::new();
        loop {
            if existing.exists() {
                break;
            }
            match existing.file_name() {
                Some(name) => {
                    remainder.push(name.to_string_lossy().to_string());
                    existing.pop();
                }
                None => return Err(internal_error("cannot resolve path")),
            }
        }
        let mut result = existing
            .canonicalize()
            .map_err(|e| internal_error(format!("cannot resolve path: {e}")))?;
        for name in remainder.iter().rev() {
            result = result.join(name);
        }
        result
    };

    // Verify the resolved path is still inside the base directory.
    if !canonical.starts_with(&canonical_base) {
        return Err(bad_request("path traversal not allowed"));
    }

    Ok(canonical)
}

/// `GET /files?path=<rel>` — streams a file from the storage directory.
async fn handle_download(
    state: Arc<FilesState>,
    headers: HeaderMap,
    query: Query<FileQuery>,
) -> Response {
    if let Err(response) = check_auth(&state.auth, &headers) {
        return response;
    }

    let base = match resolve_storage_dir(&state.core) {
        Some(b) => b,
        None => return internal_error("could not resolve storage directory"),
    };

    let path = match resolve_safe_path(&base, &query.path) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    if !path.is_file() {
        return not_found(format!("not a file: {}", query.path));
    }

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    mlog!("file download: {}", query.path);

    // Read the file and serve it with Content-Disposition: attachment.
    // (axum 0.7 has no built-in serve_file; files are typically small enough
    // to buffer, and this keeps the implementation simple.)
    let data = match tokio::fs::read(&path).await {
        Ok(d) => d,
        Err(e) => return internal_error(format!("failed to read file: {e}")),
    };

    let response = (
        StatusCode::OK,
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        )],
        data,
    )
        .into_response();
    add_cors_headers(response)
}

/// `POST /files` — multipart upload.
///
/// Expected fields:
/// - `file` — the file content (binary).
/// - `target` — relative path within the storage directory (e.g.
///   `prod_db_csv/import.csv`). Parent directories are created as needed.
async fn handle_upload(
    state: Arc<FilesState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(response) = check_auth(&state.auth, &headers) {
        return response;
    }

    let base = match resolve_storage_dir(&state.core) {
        Some(b) => b,
        None => return internal_error("could not resolve storage directory"),
    };

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut target: Option<String> = None;

    loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return bad_request(format!("invalid multipart body: {e}")),
        };

        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let mut buf = Vec::new();
                loop {
                    let chunk = match field.chunk().await {
                        Ok(Some(c)) => c,
                        Ok(None) => break,
                        Err(e) => return bad_request(format!("read error: {e}")),
                    };
                    buf.extend_from_slice(&chunk);
                    if buf.len() as u64 > MAX_UPLOAD_BYTES {
                        return too_large();
                    }
                }
                file_bytes = Some(buf);
            }
            "target" => {
                let mut buf = Vec::new();
                loop {
                    let chunk = match field.chunk().await {
                        Ok(Some(c)) => c,
                        Ok(None) => break,
                        Err(e) => return bad_request(format!("read error: {e}")),
                    };
                    buf.extend_from_slice(&chunk);
                }
                target = Some(String::from_utf8_lossy(&buf).to_string());
            }
            _ => {
                // Ignore unknown fields.
            }
        }
    }

    let file_bytes = match file_bytes {
        Some(b) => b,
        None => return bad_request("missing 'file' field"),
    };
    let target = match target {
        Some(t) if !t.is_empty() => t,
        _ => return bad_request("missing or empty 'target' field"),
    };

    let dest = match resolve_safe_path(&base, &target) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Create parent directories.
    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return internal_error(format!("failed to create directory: {e}"));
        }
    }

    if let Err(e) = std::fs::write(&dest, &file_bytes) {
        return internal_error(format!("failed to write file: {e}"));
    }

    mlog!("file upload: {} ({} bytes)", target, file_bytes.len());

    let response = (
        StatusCode::CREATED,
        Json(json!({
            "path": target,
            "size": file_bytes.len(),
        })),
    )
        .into_response();
    add_cors_headers(response)
}

// --- Error helpers -------------------------------------------------------

fn bad_request(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": msg.into() })),
    )
        .into_response()
}

fn not_found(msg: impl Into<String>) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "error": msg.into() }))).into_response()
}

fn too_large() -> Response {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        Json(json!({ "error": format!("file exceeds {MAX_UPLOAD_BYTES} byte limit") })),
    )
        .into_response()
}

fn internal_error(msg: impl Into<String>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": msg.into() })),
    )
        .into_response()
}
