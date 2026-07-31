// crates/network/src/protocol.rs
//
// Protocol layer — the REAL logic for fetch and push.
//
// This module does NOT depend on HTTP. It operates on pure data structures
// and storage interfaces. The server and client modules translate between
// transport frames and this layer.

use serde::{Deserialize, Serialize};

// ─── Request / Response types ───────────────────────────────────────

/// Phase 1: Client requests metadata (commits, trees, blob manifests)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataRequest {
    /// Commit hashes the client wants to reach
    pub want: Vec<String>,
}

/// A single object to be transferred (hash + raw bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferObject {
    pub hash: String,
    pub data: Vec<u8>,
}

/// Phase 1: Server response with metadata objects only (no chunks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataResponse {
    pub objects: Vec<TransferObject>,
}

/// Phase 2: Client requests missing chunks by hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRequest {
    /// Chunk hashes the client needs
    pub want_chunks: Vec<String>,
}

/// Phase 2: Server response with chunk data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkResponse {
    pub chunks: Vec<TransferObject>,
}

/// Push: client sends branch metadata, then streams objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    /// The new HEAD commit hash for the branch
    pub new_head: String,
    /// Target branch name
    pub branch: String,
}

/// Push: server response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResponse {
    pub success: bool,
    pub message: String,
}

/// Ref listing returned by GET /refs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefsResponse {
    /// Map of ref name → commit hash (e.g. "refs/heads/main" → "abc123...")
    pub refs: std::collections::HashMap<String, String>,
    /// Current HEAD (commit hash or symbolic ref)
    pub head: Option<String>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_request_serialization() {
        let req = MetadataRequest {
            want: vec!["abc123".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: MetadataRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.want, req.want);
    }

    #[test]
    fn test_push_response_serialization() {
        let resp = PushResponse {
            success: true,
            message: "ok".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: PushResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
    }
}
