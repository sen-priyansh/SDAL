use crate::protocol::{
    ChunkRequest, ChunkResponse, MetadataRequest, MetadataResponse, PushRequest, PushResponse,
    RefsResponse,
};
use crate::transport::Transport;
use anyhow::Result;
use sdal_core::refs::Refs;
use sdal_storage::{FilesystemStorage, Storage};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

/// P2P Transport using custom SDALP (SDAL Peer) binary protocol over raw TCP.
pub struct P2pTransport {
    address: String,
}

impl P2pTransport {
    pub fn new(address: &str) -> Self {
        let addr = address.strip_prefix("sdalp://").unwrap_or(address).to_string();
        Self { address: addr }
    }
}

impl Transport for P2pTransport {
    fn get(&self, path: &str) -> Result<Vec<u8>> {
        let mut stream = TcpStream::connect(&self.address)?;
        writeln!(stream, "GET")?;
        writeln!(stream, "{}", path)?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        Ok(response)
    }

    fn post(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>> {
        let mut stream = TcpStream::connect(&self.address)?;
        writeln!(stream, "POST")?;
        writeln!(stream, "{}", path)?;
        stream.write_all(&body)?;

        stream.shutdown(std::net::Shutdown::Write)?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        Ok(response)
    }

    fn post_stream(
        &self,
        path: &str,
        envelope_json: Vec<u8>,
        mut body_stream: Box<dyn std::io::Read + Send + 'static>,
    ) -> Result<Box<dyn std::io::Read + Send>> {
        let mut stream = TcpStream::connect(&self.address)?;
        writeln!(stream, "POST_STREAM")?;
        writeln!(stream, "{}", path)?;
        stream.write_all(&envelope_json)?;
        stream.write_all(b"\n")?;

        std::io::copy(&mut body_stream, &mut stream)?;
        stream.shutdown(std::net::Shutdown::Write)?;

        Ok(Box::new(stream))
    }

    fn post_receive_stream(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Box<dyn std::io::Read + Send>> {
        let mut stream = TcpStream::connect(&self.address)?;
        writeln!(stream, "POST_RECEIVE_STREAM")?;
        writeln!(stream, "{}", path)?;
        stream.write_all(&body)?;

        stream.shutdown(std::net::Shutdown::Write)?;

        Ok(Box::new(stream))
    }
}

/// Start a simple P2P server to serve the local repository.
pub fn serve_p2p(repo_root: &Path, port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    let storage = FilesystemStorage::new(repo_root)?;
    let refs = Refs::new(repo_root);

    println!("Listening for peer connections on sdalp://0.0.0.0:{}", port);

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let storage_clone = FilesystemStorage::new(repo_root).unwrap();
            let refs_clone = Refs::new(repo_root);
            std::thread::spawn(move || {
                if let Err(e) = handle_p2p_connection(&mut stream, &storage_clone, &refs_clone) {
                    eprintln!("Peer error: {}", e);
                }
            });
        }
    }

    Ok(())
}

fn handle_p2p_connection(
    stream: &mut TcpStream,
    storage: &FilesystemStorage,
    refs: &Refs,
) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut method = String::new();
    reader.read_line(&mut method)?;
    let method = method.trim();

    let mut path = String::new();
    reader.read_line(&mut path)?;
    let path = path.trim();

    match method {
        "GET" => {
            if path == "/refs" {
                let current_branch = refs.get_current_branch()?.unwrap_or_else(|| "main".to_string());
                let branches = refs.list_branches()?;
                let mut all_refs = std::collections::HashMap::new();
                for b in branches {
                    let ref_name = format!("refs/heads/{}", b);
                    if let Some(h) = refs.read_ref(&ref_name)? {
                        all_refs.insert(ref_name, h);
                    }
                }
                let head_hash = refs.read_ref(&format!("refs/heads/{}", current_branch))?;
                let resp = RefsResponse {
                    refs: all_refs,
                    head: head_hash,
                };
                let resp_bytes = serde_json::to_vec(&resp)?;
                stream.write_all(&resp_bytes)?;
            }
        }
        "POST" | "POST_RECEIVE_STREAM" => {
            let mut body = Vec::new();
            reader.read_to_end(&mut body)?;

            // Parse envelope
            let envelope: crate::identity::SignedEnvelope = serde_json::from_slice(&body)?;
            // Verify signature (allow up to 300 seconds age)
            crate::identity::verify_envelope(&envelope, 300)?;
            
            if path == "/metadata/discover" {
                let req: MetadataRequest = serde_json::from_slice(&envelope.payload)?;
                let mut objects = Vec::new();

                // Simple traversal for metadata (commits, trees, blob metadata)
                let mut queue = std::collections::VecDeque::new();
                let mut visited = std::collections::HashSet::new();
                for w in req.want {
                    queue.push_back(w);
                }

                while let Some(hash) = queue.pop_front() {
                    if visited.contains(&hash) { continue; }
                    visited.insert(hash.clone());

                    if let Ok(data) = storage.get(&hash) {
                        objects.push(crate::protocol::TransferObject {
                            hash: hash.clone(),
                            data: data.clone(),
                        });

                        if let Ok(obj) = sdal_core::Object::from_bytes(&data) {
                            match obj {
                                sdal_core::Object::Commit(c) => {
                                    for p in &c.parents { queue.push_back(p.clone()); }
                                    queue.push_back(c.tree);
                                }
                                sdal_core::Object::Tree(t) => {
                                    for (_, entry) in &t.entries {
                                        match entry {
                                            sdal_core::TreeEntry::Blob { hash, .. } => queue.push_back(hash.clone()),
                                            sdal_core::TreeEntry::Tree { hash } => queue.push_back(hash.clone()),
                                        }
                                    }
                                }
                                sdal_core::Object::Blob(_) => {
                                    // Don't traverse into chunk hashes for metadata discovery!
                                }
                                sdal_core::Object::PullRequest(pr) => {
                                    queue.push_back(pr.head_commit);
                                }
                            }
                        }
                    }
                }

                let resp = MetadataResponse { objects };
                let resp_bytes = serde_json::to_vec(&resp)?;
                stream.write_all(&resp_bytes)?;

            } else if path == "/chunks/fetch" {
                let req: ChunkRequest = serde_json::from_slice(&envelope.payload)?;
                // Stream chunks
                for hash in req.want_chunks {
                    if let Ok(data) = storage.get(&hash) {
                        let frame = crate::wire::Frame {
                            frame_type: crate::wire::FrameType::Chunk,
                            data,
                        };
                        crate::wire::write_frame(stream, &frame)?;
                    }
                }
                crate::wire::write_end(stream)?;
            }
        }
        "POST_STREAM" => {
            if path == "/push" {
                let mut envelope_line = String::new();
                reader.read_line(&mut envelope_line)?;
                
                let envelope: crate::identity::SignedEnvelope = serde_json::from_str(envelope_line.trim())?;
                crate::identity::verify_envelope(&envelope, 300)?;
                
                let req: PushRequest = serde_json::from_slice(&envelope.payload)?;

                // Read streamed frames
                let mut chunks_saved = 0;
                while let Some(frame) = crate::wire::read_frame(&mut reader)? {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&frame.data);
                    let hash = hex::encode(hasher.finalize());

                    if let Err(e) = storage.put(&hash, &frame.data) {
                        if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
                            anyhow::bail!("Storage error: {}", e);
                        }
                    } else {
                        chunks_saved += 1;
                    }
                }

                // Update refs
                let local_ref = format!("refs/heads/{}", req.branch);
                refs.update_ref(&local_ref, &req.new_head)?;

                let resp = PushResponse {
                    success: true,
                    message: format!("Pushed to peer branch {}. Saved {} objects.", req.branch, chunks_saved),
                };
                let resp_bytes = serde_json::to_vec(&resp)?;
                stream.write_all(&resp_bytes)?;
            }
        }
        _ => anyhow::bail!("Unknown method {}", method),
    }

    Ok(())
}
