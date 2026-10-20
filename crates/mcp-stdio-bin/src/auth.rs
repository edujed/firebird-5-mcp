//! Simple bearer-token authentication for the MCP SSE transport.
//!
//! Authentication is **disabled by default**. When enabled (via the
//! `--token` CLI flag or `--token-file`), every request to `/sse` and
//! `/messages` must carry a valid `Authorization: Bearer <token>` header.
//!
//! This is intentionally minimal: it validates a static token (or a token
//! read from a file) against the request. It does **not** implement token
//! expiration, rotation, or user identity — those concerns are delegated to
//! an external authentication interface (as agreed with the user). The
//! `AuthConfig` struct is designed so that a more sophisticated validator
//! can be plugged in later without changing the call sites.

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Authentication configuration for the SSE transport.
///
/// When `token` is `None`, authentication is disabled and all requests are
/// allowed. When `Some`, the request must carry an `Authorization` header
/// with the value `Bearer <token>`.
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// The expected bearer token, or `None` to disable authentication.
    pub token: Option<String>,
}

impl AuthConfig {
    /// Creates a disabled auth config (all requests allowed).
    pub fn disabled() -> Self {
        Self { token: None }
    }

    /// Creates an auth config that requires the given bearer token.
    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            token: Some(token.into()),
        }
    }

    /// Returns `true` if authentication is enabled.
    pub fn is_enabled(&self) -> bool {
        self.token.is_some()
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Validates the `Authorization` header against the configured token.
///
/// Returns `Ok(())` if the request is authorized (or auth is disabled),
/// or `Err(Response)` with a `401 Unauthorized` response if the token is
/// missing or invalid.
///
/// The comparison is done in constant time to mitigate timing attacks.
pub fn check_auth(auth: &AuthConfig, headers: &HeaderMap) -> Result<(), Response> {
    let expected = match &auth.token {
        None => return Ok(()), // auth disabled
        Some(t) => t,
    };

    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            // Also accept `bearer` (lowercase) — HTTP headers are
            // case-insensitive, but the scheme prefix convention is
            // `Bearer`.
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("bearer "))
        });

    match provided {
        Some(p) if constant_time_eq(p.as_bytes(), expected.as_bytes()) => Ok(()),
        Some(_) => Err(unauthorized("Invalid token")),
        None => Err(unauthorized("Missing Authorization header")),
    }
}

/// Builds a `401 Unauthorized` response with a JSON-RPC error body.
fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32001, "message": message }
        })),
    )
        .into_response()
}

/// Constant-time string comparison to mitigate timing attacks.
///
/// Returns `true` if the two byte slices are equal. The comparison time is
/// independent of the position of the first differing byte.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    fn headers_with_auth(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        headers
    }

    #[test]
    fn test_auth_disabled_allows_all() {
        let auth = AuthConfig::disabled();
        let mut headers = HeaderMap::new();
        assert!(check_auth(&auth, &headers).is_ok());
    }

    #[test]
    fn test_auth_enabled_valid_token() {
        let auth = AuthConfig::with_token("secret");
        let headers = headers_with_auth("secret");
        assert!(check_auth(&auth, &headers).is_ok());
    }

    #[test]
    fn test_auth_enabled_invalid_token() {
        let auth = AuthConfig::with_token("secret");
        let headers = headers_with_auth("wrong");
        assert!(check_auth(&auth, &headers).is_err());
    }

    #[test]
    fn test_auth_enabled_missing_header() {
        let auth = AuthConfig::with_token("secret");
        let headers = HeaderMap::new();
        assert!(check_auth(&auth, &headers).is_err());
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
