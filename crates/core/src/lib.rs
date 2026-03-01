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
        debug_assert!(
            !name.contains('/') && !name.contains('\\'),
            "Tree entry names must not contain path separators. Got: {}",
            name
        );

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

// Binary Serialization Utilities
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub struct BinaryWriter<W: Write> {
    inner: W,
}

impl<W: Write> BinaryWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn write_u8(&mut self, v: u8) -> io::Result<()> {
        self.inner.write_all(&[v])
    }

    pub fn write_u16(&mut self, v: u16) -> io::Result<()> {
        self.inner.write_all(&v.to_le_bytes())
    }

    pub fn write_u32(&mut self, v: u32) -> io::Result<()> {
        self.inner.write_all(&v.to_le_bytes())
    }

    pub fn write_bytes(&mut self, b: &[u8]) -> io::Result<()> {
        self.inner.write_all(b)
    }
}

pub struct BinaryReader<R: Read> {
    inner: R,
}

impl<R: Read> BinaryReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    fn read_exact<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn read_u8(&mut self) -> io::Result<u8> {
        Ok(self.read_exact::<1>()?[0])
    }

    pub fn read_u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact::<2>()?))
    }

    pub fn read_u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    pub fn read_bytes(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }
}

const TREE_MAGIC: &[u8; 4] = b"SDTR";
const TREE_VERSION: u8 = 1;

impl Tree {
    pub fn write_binary<W: Write>(&self, mut w: W) -> io::Result<[u8; 32]> {
        let mut payload = Vec::new();
        {
            let mut pw = BinaryWriter::new(&mut payload);

            pw.write_u32(self.entries.len() as u32)?;

            for (name, entry) in &self.entries {
                let (t, hash_str) = match entry {
                    TreeEntry::Blob { hash, .. } => (0u8, hash),
                    TreeEntry::Tree { hash } => (1u8, hash),
                };

                let hash_bytes = hex::decode(hash_str)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

                if hash_bytes.len() != 32 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid hash length in tree entry",
                    ));
                }

                let name_bytes = name.as_bytes();
                pw.write_u8(t)?;
                pw.write_u16(name_bytes.len() as u16)?;
                pw.write_bytes(name_bytes)?;
                pw.write_bytes(&hash_bytes)?;
            }
        }

        let hash = Sha256::digest(&payload);
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(&hash);

        // write header
        let mut bw = BinaryWriter::new(&mut w);
        bw.write_bytes(TREE_MAGIC)?;
        bw.write_u8(TREE_VERSION)?;
        bw.write_u8(0)?; // flags
        bw.write_u16(0)?; // reserved
        bw.write_bytes(&payload)?;

        Ok(hash_bytes)
    }

    pub fn read_binary<R: Read>(r: R) -> io::Result<Self> {
        let mut br = BinaryReader::new(r);

        // Safety limits
        const MAX_TREE_ENTRIES: usize = 1_000_000;
        const MAX_NAME_LEN: usize = 4096;

        // ---- header ----
        let magic = br.read_bytes(4)?;
        if magic != TREE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid tree magic",
            ));
        }

        let version = br.read_u8()?;
        if version != TREE_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported tree version",
            ));
        }

        br.read_u8()?; // flags
        br.read_u16()?; // reserved

        // ---- payload ----
        let entry_count = br.read_u32()? as usize;
        if entry_count > MAX_TREE_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "tree entry count too large",
            ));
        }

        let mut entries = Vec::with_capacity(entry_count);

        for _ in 0..entry_count {
            let t = br.read_u8()?;
            let name_len = br.read_u16()? as usize;

            if name_len == 0 || name_len > MAX_NAME_LEN {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid tree entry name length",
                ));
            }

            let name_bytes = br.read_bytes(name_len)?;
            let name = String::from_utf8(name_bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8 name"))?;

            let hash_vec = br.read_bytes(32)?;
            let hash_str = hex::encode(hash_vec);

            let entry = match t {
                0 => TreeEntry::Blob {
                    hash: hash_str,
                    size: 0, // Size not present in binary format
                },
                1 => TreeEntry::Tree { hash: hash_str },
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid tree entry type",
                    ));
                }
            };

            entries.push((name, entry));
        }

        let tree = Tree { entries };

        // Enforce invariants (sorted, unique names, valid names)
        tree.validate()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(tree)
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

    /// Load object from bytes, detecting format (Binary or JSON)
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        // Check for Tree Magic "SDTR"
        if data.len() >= 4 && &data[0..4] == TREE_MAGIC {
            let tree = Tree::read_binary(data).map_err(|e| e.to_string())?;
            return Ok(Object::Tree(tree));
        }

        // Fallback to JSON
        // TODO: This fallback is temporary. Once migration is complete,
        // this should be behind a feature flag or removed to enforce binary format.
        serde_json::from_slice(data).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_binary_roundtrip() {
        let mut tree = Tree::new();
        // Add a blob entry
        let blob_hash = "cafebabe".repeat(8);
        tree.add_entry(
            "test.txt".to_string(),
            TreeEntry::Blob {
                hash: blob_hash.clone(),
                size: 123,
            },
        );

        // Add a subtree entry
        let tree_hash = "deadbeef".repeat(8);
        tree.add_entry(
            "subdir".to_string(),
            TreeEntry::Tree {
                hash: tree_hash.clone(),
            },
        );

        // Sort is required for deterministic binary format and validation
        tree.sort();

        // Write
        let mut buffer = Vec::new();
        let _ = tree.write_binary(&mut buffer).expect("write failed");

        // Verify Header
        assert_eq!(&buffer[0..4], b"SDTR"); // Magic
        assert_eq!(buffer[4], 1); // Version

        // Read
        let loaded_tree = Tree::read_binary(&buffer[..]).expect("read failed");

        // Verify entries
        assert_eq!(loaded_tree.entries.len(), 2);

        let found_blob = loaded_tree
            .entries
            .iter()
            .find(|(n, _)| n == "test.txt")
            .unwrap();
        if let TreeEntry::Blob { hash, size } = &found_blob.1 {
            assert_eq!(hash, &blob_hash);
            assert_eq!(*size, 0); // Size should be 0 from binary read
        } else {
            panic!("Expected blob");
        }

        let found_tree = loaded_tree
            .entries
            .iter()
            .find(|(n, _)| n == "subdir")
            .unwrap();
        if let TreeEntry::Tree { hash } = &found_tree.1 {
            assert_eq!(hash, &tree_hash);
        } else {
            panic!("Expected tree");
        }
    }
}
