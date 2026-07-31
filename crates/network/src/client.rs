// crates/network/src/client.rs
//
// Client-side networking logic.
//
// Translates high-level operations (push, fetch, clone) into
// Transport calls. Does NOT contain HTTP logic directly — uses
// the Transport trait abstraction.
//
// Phase 1: All outbound requests are wrapped in a SignedEnvelope
// (Ed25519 signature + timestamp + nonce) for authentication.

use crate::identity;
use crate::protocol::{
    ChunkRequest, ChunkResponse, MetadataRequest, MetadataResponse, PushRequest, PushResponse,
    RefsResponse, TransferObject,
};
use crate::transport::Transport;
use ed25519_dalek::SigningKey;
use sdal_core::Object;
use sdal_storage::{FilesystemStorage, Storage};
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};

// ─── Helpers ────────────────────────────────────────────────────────

/// Sign a payload and POST the envelope JSON to the given path.
/// Returns the raw response bytes from the server.
fn signed_post(
    transport: &dyn Transport,
    signing_key: &SigningKey,
    path: &str,
    payload: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let envelope = identity::sign_payload(signing_key, payload);
    let envelope_json = serde_json::to_vec(&envelope)?;
    transport.post(path, envelope_json)
}

// ─── Public API ─────────────────────────────────────────────────────

/// Fetch remote refs (branch → commit hash mapping).
///
/// The empty-body request is still signed to prove identity.
pub fn fetch_refs(
    transport: &dyn Transport,
    signing_key: &SigningKey,
) -> anyhow::Result<RefsResponse> {
    // Sign an empty payload — server verifies identity before returning refs
    let response_bytes = signed_post(transport, signing_key, "/refs", b"")?;
    let refs: RefsResponse = serde_json::from_slice(&response_bytes)?;
    Ok(refs)
}

/// Fetch objects from remote, storing them locally.
///
/// 1. Phase 1: Request metadata (commits, trees, blobs)
/// 2. Compute missing chunks locally
/// 3. Phase 2: Request only missing chunks
pub fn fetch(
    transport: &dyn Transport,
    storage: &FilesystemStorage,
    want: Vec<String>,
    _repo_root: &std::path::Path,
    signing_key: &SigningKey,
) -> anyhow::Result<()> {
    // 1. Phase 1: Request metadata
    let meta_req = MetadataRequest { want: want.clone() };
    let meta_req_bytes = serde_json::to_vec(&meta_req)?;

    println!("  Fetching metadata (Phase 1)...");
    let meta_resp_bytes =
        signed_post(transport, signing_key, "/metadata/discover", &meta_req_bytes)?;
    let meta_resp: MetadataResponse = serde_json::from_slice(&meta_resp_bytes)?;

    for obj in &meta_resp.objects {
        verify_and_store(storage, obj)?;
    }

    println!("  ✓ Received {} metadata objects", meta_resp.objects.len());

    // 2. Client-side graph traversal to find missing chunks
    let want_chunks = compute_missing_chunks(storage, &want)?;

    if want_chunks.is_empty() {
        println!("  ✓ All chunks already present locally. No chunk download needed.");
        return Ok(());
    }

    // 3. Phase 2: Request missing chunks
    println!("  Fetching {} missing chunks (Phase 2)...", want_chunks.len());
    let chunk_req = ChunkRequest { want_chunks };
    let chunk_req_bytes = serde_json::to_vec(&chunk_req)?;

    let chunk_resp_bytes = signed_post(transport, signing_key, "/chunks/fetch", &chunk_req_bytes)?;
    let chunk_resp: ChunkResponse = serde_json::from_slice(&chunk_resp_bytes)?;

    for obj in &chunk_resp.chunks {
        verify_and_store(storage, obj)?;
    }

    println!("  ✓ Received {} chunks", chunk_resp.chunks.len());

    Ok(())
}

fn verify_and_store(storage: &FilesystemStorage, obj: &TransferObject) -> anyhow::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(&obj.data);
    let computed = hex::encode(hasher.finalize());
    if computed != obj.hash {
        anyhow::bail!(
            "Hash mismatch from remote: expected {}, computed {}",
            obj.hash,
            computed
        );
    }
    match storage.put(&obj.hash, &obj.data) {
        Ok(_) => Ok(()),
        Err(sdal_storage::StorageError::AlreadyExists(_)) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn compute_missing_chunks(
    storage: &FilesystemStorage,
    wants: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut missing_chunks: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    for w in wants {
        queue.push_back(w.clone());
    }

    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) || hash.is_empty() {
            continue;
        }
        visited.insert(hash.clone());

        let data = match storage.get(&hash) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Ok(obj) = Object::from_bytes(&data) {
            match obj {
                Object::Commit(commit) => {
                    for parent in &commit.parents {
                        queue.push_back(parent.clone());
                    }
                    queue.push_back(commit.tree.clone());
                }
                Object::Tree(tree) => {
                    for (_, entry) in &tree.entries {
                        match entry {
                            sdal_core::TreeEntry::Blob { hash, .. } => queue.push_back(hash.clone()),
                            sdal_core::TreeEntry::Tree { hash } => queue.push_back(hash.clone()),
                        }
                    }
                }
                Object::Blob(blob) => {
                    for chunk in &blob.chunks {
                        if !storage.exists(&chunk.hash) {
                            missing_chunks.insert(chunk.hash.clone());
                        }
                    }
                }
            }
        }
    }

    Ok(missing_chunks.into_iter().collect())
}

/// Push local commits/objects to a remote.
///
/// 1. Walk local commit graph from the branch head
/// 2. Collect all reachable objects
/// 3. Sign and send PushRequest
pub fn push(
    transport: &dyn Transport,
    storage: &FilesystemStorage,
    repo_root: &std::path::Path,
    branch: &str,
    signing_key: &SigningKey,
) -> anyhow::Result<()> {
    let refs = sdal_core::refs::Refs::new(repo_root);
    let ref_name = format!("refs/heads/{}", branch);

    let head_hash = refs
        .read_ref(&ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch '{}' has no commits", branch))?;

    if head_hash.is_empty() {
        anyhow::bail!("Branch '{}' has no commits", branch);
    }

    // Walk local commit graph and collect all objects
    let objects = collect_push_objects(storage, &head_hash)?;

    println!("  Pushing {} objects to remote...", objects.len());

    let req = PushRequest {
        objects,
        new_head: head_hash.clone(),
        branch: branch.to_string(),
    };

    let req_bytes = serde_json::to_vec(&req)?;
    let response_bytes = signed_post(transport, signing_key, "/push", &req_bytes)?;
    let response: PushResponse = serde_json::from_slice(&response_bytes)?;

    if response.success {
        println!("  ✓ {}", response.message);
    } else {
        anyhow::bail!("Push failed: {}", response.message);
    }

    Ok(())
}

/// Walk the commit graph starting from `head` and collect all
/// reachable objects (commits, trees, blobs, chunks).
fn collect_push_objects(
    storage: &FilesystemStorage,
    head: &str,
) -> anyhow::Result<Vec<TransferObject>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut objects: Vec<TransferObject> = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    queue.push_back(head.to_string());

    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) || hash.is_empty() {
            continue;
        }
        visited.insert(hash.clone());

        let data = match storage.get(&hash) {
            Ok(d) => d,
            Err(_) => continue,
        };

        objects.push(TransferObject {
            hash: hash.clone(),
            data: data.clone(),
        });

        // Parse and walk graph
        if let Ok(obj) = Object::from_bytes(&data) {
            match obj {
                Object::Commit(commit) => {
                    for parent in &commit.parents {
                        if !visited.contains(parent) {
                            queue.push_back(parent.clone());
                        }
                    }
                    if !visited.contains(&commit.tree) {
                        queue.push_back(commit.tree.clone());
                    }
                }
                Object::Tree(tree) => {
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
                    for chunk_entry in &blob.chunks {
                        if !visited.contains(&chunk_entry.hash) {
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

    Ok(objects)
}
