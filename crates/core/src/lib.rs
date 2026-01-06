use serde::{Deserialize, Serialize};
use sdal_chunking::Chunk;

pub mod invariants;
pub mod refs;
pub mod workdir;
pub mod index;
pub mod ignore;
pub mod checkout;
pub mod merge;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Blob {
    pub chunks: Vec<ChunkEntry>,
    pub total_size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChunkEntry {
    pub hash: String,
    pub size: u64,
    pub offset: u64,
}

impl From<&Chunk> for ChunkEntry {
    fn from(chunk: &Chunk) -> Self {
        Self {
            hash: chunk.hash.clone(),
            size: chunk.data.len() as u64,
            offset: chunk.offset,
        }
    }
}

impl Blob {
    /// Validate blob invariants
    pub fn validate(&self) -> Result<(), String> {
        // Invariant 1: Total size must match sum of chunk sizes
        let computed_size: u64 = self.chunks.iter().map(|c| c.size).sum();
        if computed_size != self.total_size {
            return Err(format!(
                "Blob size mismatch: total_size={}, computed={}",
                self.total_size, computed_size
            ));
        }
        
        // Invariant 2: Chunk offsets must be sequential
        let mut expected_offset = 0u64;
        for (i, chunk) in self.chunks.iter().enumerate() {
            if chunk.offset != expected_offset {
                return Err(format!(
                    "Chunk {} offset mismatch: expected {}, got {}",
                    i, expected_offset, chunk.offset
                ));
            }
            expected_offset += chunk.size;
        }
        
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TreeEntry {
    Blob { hash: String, size: u64 },
    Tree { hash: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tree {
    pub entries: Vec<(String, TreeEntry)>, // (name, entry)
}

impl Tree {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }
    
    pub fn add_entry(&mut self, name: String, entry: TreeEntry) {
        self.entries.push((name, entry));
    }
    
    /// Validate tree invariants
    pub fn validate(&self) -> Result<(), String> {
        // Invariant: No duplicate names
        let mut names = std::collections::HashSet::new();
        for (name, _) in &self.entries {
            if !names.insert(name) {
                return Err(format!("Duplicate entry name: {}", name));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
    pub tree: String,           // hash of root tree
    pub parents: Vec<String>,   // parent commit hashes (0=initial, 1=normal, 2=merge)
    pub author: String,
    pub message: String,
    pub timestamp: i64,         // Unix timestamp
}

impl Commit {
    /// Validate commit invariants
    pub fn validate(&self) -> Result<(), String> {
        if self.tree.is_empty() {
            return Err("Commit must have a tree".to_string());
        }
        if self.message.is_empty() {
            return Err("Commit must have a message".to_string());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
}

impl Object {
    /// Validate object invariants
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Object::Blob(blob) => blob.validate(),
            Object::Tree(tree) => tree.validate(),
            Object::Commit(commit) => commit.validate(),
        }
    }
}
