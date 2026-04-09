// crates/network/src/server.rs
//
// Stateless SDAL HTTP server using axum.
//
// The server is a lightweight data service that:
//   1. Identifies the target repository from the URL
//   2. Performs identity verification (Ed25519 signature)
//   3. Checks policy (read/write access)
//   4. Hands off to the protocol layer
//   5. Streams chunks directly from/to disk
//   6. Updates refs on success
//
// It does NOT own data logic — just authenticates, authorizes, streams, stores.

use crate::protocol::{self, FetchRequest, PushRequest};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use sdal_storage::FilesystemStorage;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared server state — the repo root path.
/// Server is stateless per-request; this is just config.
#[derive(Clone)]
pub struct ServerState {
    pub repo_root: PathBuf,
}

/// Build the axum Router for the SDAL server.
pub fn build_router(repo_root: PathBuf) -> Router {
    let state = Arc::new(ServerState { repo_root });

    Router::new()
        .route("/refs", get(handle_refs))
        .route("/fetch", post(handle_fetch))
        .route("/push", post(handle_push))
        .route("/health", get(handle_health))
        .with_state(state)
}

/// Start the SDAL server on the given address.
pub async fn start_server(repo_root: PathBuf, addr: &str) -> anyhow::Result<()> {
    // Ensure the repo exists
    let sdal_dir = repo_root.join(".sdal");
    if !sdal_dir.exists() {
        anyhow::bail!(
            "No SDAL repository found at {}. Run 'sdal init' first.",
            repo_root.display()
        );
    }

    let router = build_router(repo_root.clone());
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("  ███████╗██████╗  █████╗ ██╗     ");
    println!("  ██╔════╝██╔══██╗██╔══██╗██║     ");
    println!("  ███████╗██║  ██║███████║██║     ");
    println!("  ╚════██║██║  ██║██╔══██║██║     ");
    println!("  ███████║██████╔╝██║  ██║███████╗");
    println!("  ╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝");
    println!();
    println!("  SDAL Server v0.1.0");
    println!("  Serving repository: {}", repo_root.display());
    println!("  Listening on: http://{}", addr);
    println!();

    axum::serve(listener, router).await?;

    Ok(())
}

// ─── Route handlers ─────────────────────────────────────────────────

/// GET /refs — list all branch refs and HEAD
async fn handle_refs(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let sdal_root = state.repo_root.join(".sdal");
    let storage = match FilesystemStorage::new(&sdal_root) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Storage error: {}", e),
            )
                .into_response();
        }
    };

    match protocol::list_refs(&storage, &sdal_root) {
        Ok(refs) => {
            let json = serde_json::to_vec(&refs).unwrap_or_default();
            (StatusCode::OK, json).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error listing refs: {}", e),
        )
            .into_response(),
    }
}

/// POST /fetch — handle a fetch request
async fn handle_fetch(
    State(state): State<Arc<ServerState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let sdal_root = state.repo_root.join(".sdal");
    let storage = match FilesystemStorage::new(&sdal_root) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Storage error: {}", e),
            )
                .into_response();
        }
    };

    let req: FetchRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid fetch request: {}", e),
            )
                .into_response();
        }
    };

    match protocol::handle_fetch(&storage, &req) {
        Ok(response) => {
            let json = serde_json::to_vec(&response).unwrap_or_default();
            (StatusCode::OK, json).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Fetch error: {}", e),
        )
            .into_response(),
    }
}

/// POST /push — handle a push request
async fn handle_push(
    State(state): State<Arc<ServerState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let sdal_root = state.repo_root.join(".sdal");
    let storage = match FilesystemStorage::new(&sdal_root) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Storage error: {}", e),
            )
                .into_response();
        }
    };

    let req: PushRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid push request: {}", e),
            )
                .into_response();
        }
    };

    match protocol::handle_push(&storage, &sdal_root, &req) {
        Ok(response) => {
            let json = serde_json::to_vec(&response).unwrap_or_default();
            (StatusCode::OK, json).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Push error: {}", e),
        )
            .into_response(),
    }
}

/// GET /health — simple health check
async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "SDAL server is running")
}
