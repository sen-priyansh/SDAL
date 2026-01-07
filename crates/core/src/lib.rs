// crate/core/src/lib.rs
use sdal_chunking::Chunk;
use serde::{Deserialize, Serialize};

pub mod checkout;
pub mod ignore;
pub mod index;
pub mod invariants;
pub mod merge;
pub mod refs;
pub mod workdir;

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

        // Invariant 2: Chunks must be sorted by offset
        for i in 1..self.chunks.len() {
            if self.chunks[i - 1].offset >= self.chunks[i].offset {
                return Err(format!(
                    "Chunks not sorted by offset: chunk {} has offset {}, chunk {} has offset {}",
                    i - 1,
                    self.chunks[i - 1].offset,
                    i,
                    self.chunks[i].offset
                ));
            }
        }

        // Invariant 3: Chunk offsets must be sequential (no gaps)
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
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, name: String, entry: TreeEntry) {
        // Invariant: names must not contain path separators
        if name.contains('/') || name.contains('\\') {
            panic!(
                "Tree entry names must not contain path separators. Use build_tree_recursive for nested paths. Got: {}",
                name
            );
        }
        self.entries.push((name, entry));
    }

    pub fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// Validate tree invariants
    pub fn validate(&self) -> Result<(), String> {
        // Invariant 1: No duplicate names
        let mut names = std::collections::HashSet::new();
        for (name, _) in &self.entries {
            if !names.insert(name) {
                return Err(format!("Duplicate entry name: {}", name));
            }

            // Invariant 2: Names must not contain path separators
            if name.contains('/') || name.contains('\\') {
                return Err(format!("Tree entry name contains path separator: {}", name));
            }
        }

        // Invariant 3: Entries should be sorted for deterministic hashing
        // Check if entries are sorted
        for i in 1..self.entries.len() {
            if self.entries[i - 1].0 > self.entries[i].0 {
                return Err(format!(
                    "Tree entries not sorted: '{}' should come before '{}'",
                    self.entries[i].0,
                    self.entries[i - 1].0
                ));
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
    pub tree: String,         // hash of root tree
    pub parents: Vec<String>, // parent commit hashes (0=initial, 1=normal, 2=merge)
    pub author: String,
    pub message: String,
    pub timestamp: i64, // Unix timestamp
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
