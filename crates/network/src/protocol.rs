// crates/network/src/protocol.rs
//
// Protocol layer — the REAL logic for fetch and push.
//
// This module does NOT depend on HTTP. It operates on pure data structures
// and storage interfaces. The server and client modules translate between
// transport frames and this layer.

use sdal_core::{Object};
use sdal_storage::{FilesystemStorage, Storage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};

// ─── Request / Response types ───────────────────────────────────────

/// Fetch: client tells server what it wants and what it already has.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRequest {
    /// Commit hashes the client wants to reach
    pub want: Vec<String>,
    /// Chunk hashes the client already has (for dedup transfer)
    pub have_chunks: Vec<String>,
}

/// A single object to be transferred (hash + raw bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferObject {
    pub hash: String,
    pub data: Vec<u8>,
}

/// Fetch: server response with all missing objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    /// All objects the client needs (commits, trees, blobs, chunks)
    pub objects: Vec<TransferObject>,
}

/// Push: client sends objects and declares the new branch head.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    /// All objects being pushed (commits, trees, blobs, chunks)
    pub objects: Vec<TransferObject>,
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

// ─── Server-side protocol logic ─────────────────────────────────────

/// List all refs in a repository.
pub fn list_refs(_storage: &FilesystemStorage, repo_root: &std::path::Path) -> anyhow::Result<RefsResponse> {
    let refs = sdal_core::refs::Refs::new(repo_root);
    let head = refs.read_head()?;

    let mut ref_map = std::collections::HashMap::new();
    let branches = refs.list_branches()?;
    for branch in branches {
        let ref_name = format!("refs/heads/{}", branch);
        if let Some(hash) = refs.read_ref(&ref_name)? {
            ref_map.insert(ref_name, hash);
        }
    }

    Ok(RefsResponse {
        refs: ref_map,
        head,
    })
}

/// Server-side fetch: walk the commit graph from `want` hashes,
/// collect all reachable objects, exclude those the client already has.
pub fn handle_fetch(
    storage: &FilesystemStorage,
    req: &FetchRequest,
) -> anyhow::Result<FetchResponse> {
    let have_set: HashSet<&str> = req.have_chunks.iter().map(|s| s.as_str()).collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut objects: Vec<TransferObject> = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Start from wanted commit hashes
    for want_hash in &req.want {
        if !visited.contains(want_hash) {
            queue.push_back(want_hash.clone());
        }
    }

    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) || hash.is_empty() {
            continue;
        }
        visited.insert(hash.clone());

        // Try to read this object from storage
        let data = match storage.get(&hash) {
            Ok(d) => d,
            Err(_) => continue, // Object not found, skip
        };

        // Always send the object (commit, tree, blob manifest)
        objects.push(TransferObject {
            hash: hash.clone(),
            data: data.clone(),
        });

        // Parse to walk the graph
        if let Ok(obj) = Object::from_bytes(&data) {
            match obj {
                Object::Commit(commit) => {
                    // Follow parents
                    for parent in &commit.parents {
                        if !visited.contains(parent) {
                            queue.push_back(parent.clone());
                        }
                    }
                    // Follow tree
                    if !visited.contains(&commit.tree) {
                        queue.push_back(commit.tree.clone());
                    }
                }
                Object::Tree(tree) => {
                    // Follow all tree entries
                    for (_name, entry) in &tree.entries {
                        let entry_hash = match entry {
                            sdal_core::TreeEntry::Blob { hash, .. } => hash,
                            sdal_core::TreeEntry::Tree { hash } => hash,
                        };
                        if !visited.contains(entry_hash) {
                            queue.push_back(entry_hash.clone());
                        }
                    }
                }
                Object::Blob(blob) => {
                    // Collect chunk data (skip chunks the client already has)
                    for chunk_entry in &blob.chunks {
                        if !have_set.contains(chunk_entry.hash.as_str())
                            && !visited.contains(&chunk_entry.hash)
                        {
                            visited.insert(chunk_entry.hash.clone());
                            if let Ok(chunk_data) = storage.get(&chunk_entry.hash) {
                                objects.push(TransferObject {
                                    hash: chunk_entry.hash.clone(),
                                    data: chunk_data,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(FetchResponse { objects })
}

/// Server-side push: validate and store all incoming objects,
/// then update the branch ref.
pub fn handle_push(
    storage: &FilesystemStorage,
    repo_root: &std::path::Path,
    req: &PushRequest,
) -> anyhow::Result<PushResponse> {
    // 1. Validate every object: hash(data) == declared hash
    for obj in &req.objects {
        let mut hasher = Sha256::new();
        hasher.update(&obj.data);
        let computed = hex::encode(hasher.finalize());
        if computed != obj.hash {
            return Ok(PushResponse {
                success: false,
                message: format!(
                    "Hash mismatch: expected {}, computed {}",
                    obj.hash, computed
                ),
            });
        }
    }

    // 2. Store all objects (skip if already exists — dedup)
    for obj in &req.objects {
        match storage.put(&obj.hash, &obj.data) {
            Ok(_) => {}
            Err(sdal_storage::StorageError::AlreadyExists(_)) => {
                // Deduplication is working — this is fine
            }
            Err(e) => {
                return Ok(PushResponse {
                    success: false,
                    message: format!("Storage error: {}", e),
                });
            }
        }
    }

    // 3. Verify the new_head commit actually exists in storage now
    if storage.get(&req.new_head).is_err() {
        return Ok(PushResponse {
            success: false,
            message: format!("New HEAD commit {} not found in storage after push", req.new_head),
        });
    }

    // 4. Update branch ref
    let refs = sdal_core::refs::Refs::new(repo_root);
    let ref_name = format!("refs/heads/{}", req.branch);

    // Create branch if it doesn't exist, otherwise update
    match refs.read_ref(&ref_name)? {
        Some(_) => {
            refs.update_ref(&ref_name, &req.new_head)?;
        }
        None => {
            // Branch doesn't exist yet — create it by writing the ref file
            refs.update_ref(&ref_name, &req.new_head)?;
        }
    }

    Ok(PushResponse {
        success: true,
        message: format!("Updated {} to {}", req.branch, &req.new_head[..7]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_request_serialization() {
        let req = FetchRequest {
            want: vec!["abc123".to_string()],
            have_chunks: vec!["def456".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: FetchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.want, req.want);
        assert_eq!(parsed.have_chunks, req.have_chunks);
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
