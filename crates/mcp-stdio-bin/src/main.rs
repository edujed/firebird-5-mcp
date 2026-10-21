//! MCP server for Firebird.
//!
//! Supports two transports, selected via CLI flags:
//!
//! - **stdio** (default) — JSON-RPC 2.0, line-delimited, over stdin/stdout.
//!   This is the standard MCP transport for IDEs and CLI tools.
//! - **SSE** (`--sse`) — HTTP+SSE (spec `2024-11-05`), for web UIs and
//!   clients that don't support stdio (llama.cpp `--ui-mcp-proxy`, CODEX,
//!   ClaudeCode, etc.).
//!
//! All tool handlers and the protocol routing logic live in the sibling
//! crate `firebird-5-mcp-tools` (`crates/mcp-tools`). This crate is a thin
//! transport layer.

mod auth;
mod files;
mod sse;

use auth::AuthConfig;
use firebird_5_mcp_core::{Config, mlog};
use firebird_5_mcp_tools::{ProtocolCore, build_time};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

/// CLI arguments for the MCP server.
struct Args {
    /// Enable the SSE transport (instead of stdio).
    sse: bool,
    /// Bind address for the SSE server (default: `0.0.0.0`).
    host: String,
    /// Bind port for the SSE server (default: `9000`).
    port: u16,
    /// Bearer token for SSE authentication (optional).
    token: Option<String>,
    /// Path to a file containing the bearer token (optional, overrides `--token`).
    token_file: Option<String>,
    /// Session inactivity timeout in minutes for the SSE transport (default: 30).
    session_timeout_min: u64,
    /// Explicit base URL for the `/files` download endpoint (e.g.
    /// `http://192.168.1.50:9000`). When omitted, it is derived from
    /// `--host`/`--port` (SSE mode only).
    base_url: Option<String>,
}

/// Parses command-line arguments.
///
/// Supported flags:
/// - `--sse` — enable the SSE transport.
/// - `--host <addr>` — bind address for SSE (default: `0.0.0.0`).
/// - `--port <port>` — bind port for SSE (default: `9000`).
/// - `--token <token>` — bearer token for SSE authentication.
/// - `--token-file <path>` — path to a file containing the bearer token.
/// - `--session-timeout <min>` — session inactivity timeout in minutes (default: 30).
/// - `--help` / `-h` — print usage and exit.
fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut result = Args {
        sse: false,
        host: "0.0.0.0".to_string(),
        port: 9000,
        token: None,
        token_file: None,
        session_timeout_min: 30,
        base_url: None,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sse" => result.sse = true,
            "--host" => {
                i += 1;
                if i >= args.len() {
                    return Err("--host requires a value".to_string());
                }
                result.host = args[i].clone();
            }
            "--port" => {
                i += 1;
                if i >= args.len() {
                    return Err("--port requires a value".to_string());
                }
                result.port = args[i]
                    .parse()
                    .map_err(|_| format!("invalid port: {}", args[i]))?;
            }
            "--token" => {
                i += 1;
                if i >= args.len() {
                    return Err("--token requires a value".to_string());
                }
                result.token = Some(args[i].clone());
            }
            "--token-file" => {
                i += 1;
                if i >= args.len() {
                    return Err("--token-file requires a value".to_string());
                }
                result.token_file = Some(args[i].clone());
            }
            "--session-timeout" => {
                i += 1;
                if i >= args.len() {
                    return Err("--session-timeout requires a value".to_string());
                }
                result.session_timeout_min = args[i]
                    .parse()
                    .map_err(|_| format!("invalid session timeout: {}", args[i]))?;
            }
            "--base-url" => {
                i += 1;
                if i >= args.len() {
                    return Err("--base-url requires a value".to_string());
                }
                result.base_url = Some(args[i].clone());
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(format!("unknown argument: {other} (see --help)"));
            }
        }
        i += 1;
    }

    Ok(result)
}

/// Prints the usage message.
fn print_usage() {
    println!(
        "firebird-5-mcp — MCP server for Firebird (build {})\n\
         \n\
         USAGE:\n\
         \x20 firebird-5-mcp [OPTIONS]\n\
         \n\
         TRANSPORT:\n\
         \x20 (no flags)        stdio — JSON-RPC over stdin/stdout (default)\n\
         \x20 --sse             HTTP+SSE — for web UIs and non-stdio clients\n\
         \n\
         SSE OPTIONS:\n\
         \x20 --host <addr>       bind address (default: 0.0.0.0)\n\
         \x20 --port <port>       bind port (default: 9000)\n\
         \x20 --token <token>     bearer token for authentication\n\
         \x20 --token-file <path> path to a file containing the bearer token\n\
         \x20 --session-timeout <min>  session inactivity timeout (default: 30)\n\
         \x20 --base-url <url>    base URL for /files download links (default: derived from --host/--port)\n\
         \n\
         EXAMPLES:\n\
         \x20 firebird-5-mcp                          # stdio mode\n\
         \x20 firebird-5-mcp --sse --port 9000        # SSE mode on port 9000\n\
         \x20 firebird-5-mcp --sse --token s3cret     # SSE mode with auth",
        build_time()
    );
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Run with --help for usage.");
            std::process::exit(1);
        }
    };

    let cfg = match Config::load_or_create() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    // Initialize file logging to {base}/logs/ (see paths module)
    let log_dir = firebird_5_mcp_core::paths::logs_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    firebird_5_mcp_core::log::init(log_dir.to_str().unwrap());
    mlog!("MCP server started (build {})", build_time());
    mlog!(
        "config: host={} port={} user={} dbs=[{}] allowed_paths=[{}]",
        cfg.server.host,
        cfg.server.port,
        cfg.server.user,
        cfg.server.database_names(),
        cfg.server.allowed_paths.join(", ")
    );

    // In SSE mode, file-generating tools advertise a `download_url` pointing
    // at the `/files` endpoint. The base URL is `--base-url` if given, otherwise
    // derived from `--host`/`--port` (skipped when the bind host is a wildcard,
    // since that is not a usable URL host).
    let files_base_url: Option<String> = if args.sse {
        match &args.base_url {
            Some(url) => {
                let trimmed = url.trim_end_matches('/');
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => {
                if args.host == "0.0.0.0" || args.host == "::" || args.host.is_empty() {
                    None
                } else {
                    Some(format!("http://{}:{}", args.host, args.port))
                }
            }
        }
    } else {
        None
    };

    let core = Arc::new(ProtocolCore::new(cfg, files_base_url.clone()));

    if args.sse {
        // Resolve the auth token (token-file overrides --token).
        let auth = resolve_auth(&args);
        mlog!(
            "SSE mode: host={} port={} auth={} session_timeout={}min files_base_url={}",
            args.host,
            args.port,
            if auth.is_enabled() {
                "enabled"
            } else {
                "disabled"
            },
            args.session_timeout_min,
            files_base_url.as_deref().unwrap_or("<none>")
        );
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        if let Err(e) = rt.block_on(sse::run_sse(
            core,
            &args.host,
            args.port,
            Arc::new(auth),
            args.session_timeout_min,
        )) {
            eprintln!("SSE server error: {e}");
            std::process::exit(1);
        }
    } else {
        run_stdio(core);
    }
}

/// Resolves the auth configuration from the CLI args.
///
/// `--token-file` takes precedence over `--token`. If neither is provided,
/// authentication is disabled.
fn resolve_auth(args: &Args) -> AuthConfig {
    if let Some(path) = &args.token_file {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let token = content.trim().to_string();
                if token.is_empty() {
                    eprintln!("Warning: token file {path} is empty — auth disabled");
                    AuthConfig::disabled()
                } else {
                    AuthConfig::with_token(token)
                }
            }
            Err(e) => {
                eprintln!("Failed to read token file {path}: {e} — auth disabled");
                AuthConfig::disabled()
            }
        }
    } else if let Some(token) = &args.token {
        AuthConfig::with_token(token.clone())
    } else {
        AuthConfig::disabled()
    }
}

/// Runs the stdio transport: reads JSON-RPC lines from stdin and writes
/// responses to stdout.
fn run_stdio(core: Arc<ProtocolCore>) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                mlog!("ignoring malformed JSON line: {e}");
                continue;
            }
        };

        if let Some(response) = core.handle_request(&request) {
            let mut out = response.to_string();
            out.push('\n');
            if writer.write_all(out.as_bytes()).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    }
}
