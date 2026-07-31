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
    filter: Option<String>,
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
    let want_chunks = compute_missing_chunks(storage, &want, filter.as_deref())?;

    if want_chunks.is_empty() {
        println!("  ✓ All chunks already present locally. No chunk download needed.");
        return Ok(());
    }

    // 3. Phase 2: Request missing chunks
    println!("  Fetching {} missing chunks (Phase 2)...", want_chunks.len());
    let chunk_req = ChunkRequest { want_chunks };
    let chunk_req_bytes = serde_json::to_vec(&chunk_req)?;

    // Send the signed envelope and receive a stream of wire::Frame
    let envelope = identity::sign_payload(signing_key, &chunk_req_bytes);
    let envelope_json = serde_json::to_vec(&envelope)?;
    
    let mut response_stream = transport.post_receive_stream("/chunks/fetch", envelope_json)?;

    let mut chunks_received = 0;
    while let Some(frame) = crate::wire::read_frame(&mut response_stream)? {
        if frame.frame_type != crate::wire::FrameType::Chunk {
            anyhow::bail!("Expected Chunk frame, got {:?}", frame.frame_type);
        }

        // The chunk hash is not sent in the frame (since the frame just contains bytes)
        // Wait! How do we know the hash of the chunk? 
        // We can just hash the data to find out!
        let mut hasher = Sha256::new();
        hasher.update(&frame.data);
        let hash = hex::encode(hasher.finalize());

        match storage.put(&hash, &frame.data) {
            Ok(_) => chunks_received += 1,
            Err(sdal_storage::StorageError::AlreadyExists(_)) => chunks_received += 1, // Deduplication
            Err(e) => return Err(e.into()),
        }
    }

    println!("  ✓ Received {} chunks", chunks_received);

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
    filter: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let mut missing_chunks: HashSet<String> = HashSet::new();
    let mut visited: HashSet<(String, std::path::PathBuf)> = HashSet::new();
    let mut queue: VecDeque<(String, std::path::PathBuf)> = VecDeque::new();

    for w in wants {
        queue.push_back((w.clone(), std::path::PathBuf::new()));
    }

    let filter_path = filter.map(std::path::Path::new);

    while let Some((hash, current_path)) = queue.pop_front() {
        if visited.contains(&(hash.clone(), current_path.clone())) || hash.is_empty() {
            continue;
        }
        visited.insert((hash.clone(), current_path.clone()));

        let data = match storage.get(&hash) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Ok(obj) = Object::from_bytes(&data) {
            match obj {
                Object::Commit(commit) => {
                    for parent in &commit.parents {
                        queue.push_back((parent.clone(), std::path::PathBuf::new()));
                    }
                    queue.push_back((commit.tree.clone(), std::path::PathBuf::new()));
                }
                Object::Tree(tree) => {
                    for (name, entry) in &tree.entries {
                        let mut next_path = current_path.clone();
                        next_path.push(name);

                        // If filter is provided, check if next_path is on the way to filter or inside filter
                        let should_traverse = match filter_path {
                            Some(f) => {
                                next_path.starts_with(f) || f.starts_with(&next_path)
                            }
                            None => true,
                        };

                        if should_traverse {
                            match entry {
                                sdal_core::TreeEntry::Blob { hash, .. } => queue.push_back((hash.clone(), next_path)),
                                sdal_core::TreeEntry::Tree { hash } => queue.push_back((hash.clone(), next_path)),
                            }
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
/// 2. Collect all reachable object hashes
/// 3. Sign the PushRequest metadata
/// 4. Stream all objects as wire::Frames
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

    // Walk local commit graph and collect all object hashes
    let object_hashes = collect_push_object_hashes(storage, &head_hash)?;

    println!("  Pushing {} objects to remote...", object_hashes.len());

    let req = PushRequest {
        new_head: head_hash.clone(),
        branch: branch.to_string(),
    };

    let req_bytes = serde_json::to_vec(&req)?;
    let envelope = identity::sign_payload(signing_key, &req_bytes);
    let envelope_json = serde_json::to_vec(&envelope)?;

    // Stream the objects to the server
    let streamer = PushStreamer::new(storage.clone(), object_hashes);
    let mut response_stream = transport.post_stream("/push", envelope_json, Box::new(streamer))?;
    
    // The server will respond with a JSON PushResponse at the end of the stream
    let mut response_bytes = Vec::new();
    use std::io::Read;
    response_stream.read_to_end(&mut response_bytes)?;

    let response: PushResponse = serde_json::from_slice(&response_bytes)?;

    if response.success {
        println!("  ✓ {}", response.message);
    } else {
        anyhow::bail!("Push failed: {}", response.message);
    }

    Ok(())
}

/// A reader that streams objects from storage as wire frames.
struct PushStreamer {
    storage: FilesystemStorage,
    hashes: Vec<String>,
    current_index: usize,
    buffer: std::io::Cursor<Vec<u8>>,
    eof: bool,
}

impl PushStreamer {
    fn new(storage: FilesystemStorage, hashes: Vec<String>) -> Self {
        Self {
            storage,
            hashes,
            current_index: 0,
            buffer: std::io::Cursor::new(Vec::new()),
            eof: false,
        }
    }

    fn fill_buffer(&mut self) -> std::io::Result<()> {
        if self.current_index >= self.hashes.len() {
            if !self.eof {
                // Write EOF frame
                let mut out = Vec::new();
                crate::wire::write_end(&mut out)?;
                self.buffer = std::io::Cursor::new(out);
                self.eof = true;
            }
            return Ok(());
        }

        let hash = &self.hashes[self.current_index];
        self.current_index += 1;

        // Read object from storage
        let data = match self.storage.get(hash) {
            Ok(d) => d,
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Failed to read object {}: {}", hash, e),
                ));
            }
        };

        // Determine frame type (chunk or object metadata)
        // A simple heuristic: if it parses as an Object, it's Object. Otherwise Chunk.
        let frame_type = if Object::from_bytes(&data).is_ok() {
            crate::wire::FrameType::Object
        } else {
            crate::wire::FrameType::Chunk
        };

        let frame = crate::wire::Frame { frame_type, data };
        let mut out = Vec::new();
        crate::wire::write_frame(&mut out, &frame)?;
        self.buffer = std::io::Cursor::new(out);

        Ok(())
    }
}

impl std::io::Read for PushStreamer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            // Try to read from the current buffer
            let n = std::io::Read::read(&mut self.buffer, buf)?;
            if n > 0 {
                return Ok(n);
            }

            // If the buffer is empty, check if we're at EOF
            if self.eof {
                return Ok(0);
            }

            // Fill the buffer with the next frame
            self.fill_buffer()?;
        }
    }
}

/// Walk the commit graph starting from `head` and collect all
/// reachable object hashes (commits, trees, blobs, chunks).
fn collect_push_object_hashes(
    storage: &FilesystemStorage,
    head: &str,
) -> anyhow::Result<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut objects: Vec<String> = Vec::new();
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

        objects.push(hash.clone());

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
                            if storage.exists(&chunk_entry.hash) {
                                objects.push(chunk_entry.hash.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(objects)
}
