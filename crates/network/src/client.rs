// crates/network/src/client.rs
//
// Client-side networking logic.
//
// Translates high-level operations (push, fetch, clone) into
// Transport calls. Does NOT contain HTTP logic directly — uses
// the Transport trait abstraction.

use crate::protocol::{
    FetchRequest, FetchResponse, PushRequest, PushResponse, RefsResponse, TransferObject,
};
use crate::transport::Transport;
use sdal_core::{Object};
use sdal_storage::{FilesystemStorage, Storage};
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};

/// Fetch remote refs (branch → commit hash mapping).
pub fn fetch_refs(transport: &dyn Transport) -> anyhow::Result<RefsResponse> {
    let data = transport.get("/refs")?;
    let refs: RefsResponse = serde_json::from_slice(&data)?;
    Ok(refs)
}

/// Fetch objects from remote, storing them locally.
///
/// 1. Query remote refs
/// 2. Determine what local storage already has
/// 3. Send FetchRequest with want/have
/// 4. Receive and store missing objects
pub fn fetch(
    transport: &dyn Transport,
    storage: &FilesystemStorage,
    want: Vec<String>,
    _repo_root: &std::path::Path,
) -> anyhow::Result<()> {
    // Collect local chunk hashes for dedup negotiation
    // For now, send an empty have list (full fetch)
    // TODO: Phase 2 — walk local commits to build have_chunks
    let have_chunks: Vec<String> = Vec::new();

    let req = FetchRequest { want, have_chunks };
    let req_bytes = serde_json::to_vec(&req)?;

    let response_bytes = transport.post("/fetch", req_bytes)?;
    let response: FetchResponse = serde_json::from_slice(&response_bytes)?;

    // Store all received objects
    for obj in &response.objects {
        // Verify hash before storing (never trust remote)
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
            Ok(_) => {}
            Err(sdal_storage::StorageError::AlreadyExists(_)) => {
                // Already have this object locally
            }
            Err(e) => return Err(e.into()),
        }
    }

    println!(
        "  ✓ Received {} objects from remote",
        response.objects.len()
    );

    Ok(())
}

/// Push local commits/objects to a remote.
///
/// 1. Walk local commit graph from the branch head
/// 2. Collect all reachable objects
/// 3. Send PushRequest
pub fn push(
    transport: &dyn Transport,
    storage: &FilesystemStorage,
    repo_root: &std::path::Path,
    branch: &str,
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
    let response_bytes = transport.post("/push", req_bytes)?;
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
