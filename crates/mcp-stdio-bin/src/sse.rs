//! MCP HTTP+SSE transport (spec `2024-11-05`).
//!
//! Implements the legacy two-endpoint SSE transport defined in the MCP
//! protocol revision `2024-11-05`:
//!
//! - `GET /sse` — opens a Server-Sent Events stream. The first event is
//!   `endpoint`, carrying the URL of the message endpoint for this session.
//!   Subsequent events carry JSON-RPC responses and notifications.
//! - `POST /messages?sessionId=<uuid>` — receives a JSON-RPC request,
//!   processes it via [`ProtocolCore`], and delivers the response to the
//!   session's SSE stream.
//!
//! Each SSE connection is a **session**: a UUID mapped to a
//! `tokio::sync::broadcast` channel. The `POST` handler looks up the
//! session by UUID and sends the response through the channel.
//!
//! This transport is required by clients such as llama.cpp
//! (`--ui-mcp-proxy`), CODEX, and ClaudeCode, which do not yet support the
//! newer Streamable HTTP transport.

use crate::auth::{AuthConfig, check_auth};
use crate::files;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use firebird_5_mcp_core::mlog;
use firebird_5_mcp_tools::ProtocolCore;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

/// A single MCP SSE session.
///
/// Created when a client opens `GET /sse`. The `tx` is the broadcast
/// sender used to push JSON-RPC responses to the client's SSE stream.
struct Session {
    tx: broadcast::Sender<Value>,
    last_active: std::time::Instant,
}

impl Session {
    fn new() -> (Self, broadcast::Receiver<Value>) {
        let (tx, rx) = broadcast::channel(64);
        (
            Self {
                tx,
                last_active: std::time::Instant::now(),
            },
            rx,
        )
    }
}

/// Shared state for the SSE server: the protocol core, the session map,
/// and the authentication configuration.
///
/// Wrapped in `Arc` and passed to handlers via axum's `State` extractor.
#[derive(Clone)]
pub struct SseState {
    core: Arc<ProtocolCore>,
    sessions: Arc<Mutex<HashMap<Uuid, Session>>>,
    auth: Arc<AuthConfig>,
}

/// Query parameters for the `POST /messages` endpoint.
///
/// The spec uses `sessionId` (camelCase) in the query string, so we rename
/// the field to match.
#[derive(Deserialize)]
struct MessageQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// Builds the axum router for the MCP SSE transport.
///
/// # Arguments
/// * `core` — the protocol core (shared with the stdio transport).
/// * `auth` — the authentication configuration (may be disabled).
pub fn build_router(core: Arc<ProtocolCore>, auth: Arc<AuthConfig>) -> Router {
    let state = SseState {
        core,
        sessions: Arc::new(Mutex::new(HashMap::new())),
        auth,
    };

    Router::new()
        .route(
            "/sse",
            get(|State(state): State<Arc<SseState>>, headers: HeaderMap| {
                handle_sse(state, headers)
            }),
        )
        .route(
            "/messages",
            post(
                |State(state): State<Arc<SseState>>,
                 headers: HeaderMap,
                 query: Query<MessageQuery>,
                 body: String| { handle_message(state, headers, query, body) },
            ),
        )
        // Handle CORS preflight requests
        .route("/sse", axum::routing::options(cors_preflight))
        .route("/messages", axum::routing::options(cors_preflight))
        .with_state(Arc::new(state))
}

/// Handles CORS preflight requests (OPTIONS).
///
/// Echoes the requested headers from `Access-Control-Request-Headers` so the
/// browser knows which headers are allowed. This is required for POST requests
/// with `Content-Type: application/json` (the MCP protocol uses JSON bodies).
async fn cors_preflight(headers: HeaderMap) -> Response {
    // Echo the requested headers (e.g. "content-type, authorization").
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

/// `GET /sse` — opens a new SSE session and returns the event stream.
///
/// The first event is `endpoint`, carrying the URL of the message endpoint
/// for this session. The stream then yields JSON-RPC responses as `message`
/// events until the client disconnects.
async fn handle_sse(state: Arc<SseState>, headers: HeaderMap) -> Response {
    // Authenticate (if enabled).
    if let Err(response) = check_auth(&state.auth, &headers) {
        return response;
    }

    let session_id = Uuid::new_v4();
    let (session, mut rx) = Session::new();
    state.sessions.lock().unwrap().insert(session_id, session);
    mlog!("SSE session {session_id} opened");

    // Build the endpoint URL. The client POSTs JSON-RPC requests to this
    // URL. We use a relative path (`/messages?sessionId=...`) so it works
    // regardless of the host/port the client connected to.
    let endpoint = format!("/messages?sessionId={session_id}");

    // Build the SSE stream: first the `endpoint` event, then a bridge from
    // the broadcast channel to SSE `message` events.
    let stream = async_stream::stream! {
        // First event: the endpoint URL.
        yield Ok::<Event, std::io::Error>(
            Event::default().event("endpoint").data(endpoint)
        );

        // Bridge broadcast channel → SSE events.
        loop {
            match rx.recv().await {
                Ok(value) => {
                    yield Ok(Event::default().event("message").data(value.to_string()));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Missed some messages (channel full). Log and continue.
                    mlog!("SSE session {session_id} lagged (channel full)");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // All senders dropped — session is over.
                    break;
                }
            }
        }
    };

    let response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response();
    add_cors_headers(response)
}

/// `POST /messages?sessionId=<uuid>` — receives a JSON-RPC request and
/// delivers the response to the session's SSE stream.
async fn handle_message(
    state: Arc<SseState>,
    headers: HeaderMap,
    query: Query<MessageQuery>,
    body: String,
) -> Response {
    // Authenticate (if enabled).
    if let Err(response) = check_auth(&state.auth, &headers) {
        return response;
    }

    // Parse the session ID.
    let session_id: Uuid = match query.session_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32600, "message": "Invalid or missing sessionId" }
                })),
            )
                .into_response();
        }
    };

    // Parse the JSON-RPC request.
    let request: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                })),
            )
                .into_response();
        }
    };

    // Look up the session. If it doesn't exist (expired or never created),
    // return an error.
    let tx = {
        let mut sessions = state.sessions.lock().unwrap();
        match sessions.get_mut(&session_id) {
            Some(session) => {
                session.last_active = std::time::Instant::now();
                session.tx.clone()
            }
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id").cloned().unwrap_or(Value::Null),
                        "error": { "code": -32001, "message": "Session not found or expired" }
                    })),
                )
                    .into_response();
            }
        }
    };

    // Process the request. The `dispatch()` call is synchronous and can be
    // slow (database I/O), so we run it in a blocking task to avoid stalling
    // the async runtime.
    let core = state.core.clone();
    let response = tokio::task::spawn_blocking(move || core.handle_request(&request))
        .await
        .unwrap_or(None);

    // If the request was a notification (no `id`), `handle_request` returns
    // `None` — no response to send. The POST still returns 202 Accepted.
    if let Some(response) = response {
        // Send the response to the session's SSE stream. If the receiver is
        // gone (client disconnected), the send fails silently — that's fine.
        let _ = tx.send(response);
    }

    // Per the MCP SSE spec, the POST returns 202 Accepted with an empty body.
    // The actual response is delivered via the SSE stream.
    let response = StatusCode::ACCEPTED.into_response();
    add_cors_headers(response)
}

/// Starts the background session cleanup task.
///
/// Reaps sessions that have been inactive for longer than `timeout`.
fn spawn_session_cleanup(
    sessions: Arc<Mutex<HashMap<Uuid, Session>>>,
    timeout: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            let mut to_remove = Vec::new();
            {
                let mut map = sessions.lock().unwrap();
                map.retain(|id, session| {
                    if now.duration_since(session.last_active) > timeout {
                        to_remove.push(*id);
                        false
                    } else {
                        true
                    }
                });
            }
            for id in &to_remove {
                mlog!("SSE session {id} reaped (inactive for > {timeout:?})");
            }
        }
    })
}

/// Runs the MCP SSE server on the given host and port.
///
/// # Arguments
/// * `core` — the protocol core (shared with the stdio transport).
/// * `host` — the bind address (e.g. `"0.0.0.0"`).
/// * `port` — the bind port (e.g. `9000`).
/// * `auth` — the authentication configuration (may be disabled).
/// * `session_timeout_min` — session inactivity timeout in minutes.
pub async fn run_sse(
    core: Arc<ProtocolCore>,
    host: &str,
    port: u16,
    auth: Arc<AuthConfig>,
    session_timeout_min: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let sessions = Arc::new(Mutex::new(HashMap::new()));
    let router = build_router(core.clone(), auth.clone());
    let files_router = files::build_files_router(core, auth);

    // Spawn the background session cleanup task.
    let timeout = Duration::from_secs(session_timeout_min * 60);
    spawn_session_cleanup(sessions, timeout);

    let addr = format!("{host}:{port}");
    mlog!("MCP SSE server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router.merge(files_router)).await?;

    Ok(())
}
