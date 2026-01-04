use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub data: Vec<u8>,
    pub hash: String,
    pub offset: u64,
}

impl Chunk {
    pub fn new(data: Vec<u8>, offset: u64) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hex::encode(hasher.finalize());
        
        let chunk = Self { data, hash, offset };
        
        // Invariant: hash must match data
        chunk.verify().expect("Chunk creation failed: hash mismatch");
        
        chunk
    }
    
    /// Verify that the hash matches the data
    pub fn verify(&self) -> Result<(), ChunkingError> {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        let computed_hash = hex::encode(hasher.finalize());
        
        if computed_hash != self.hash {
            return Err(ChunkingError::Other(format!(
                "Chunk integrity violation: expected hash {}, got {}",
                self.hash, computed_hash
            )));
        }
        
        Ok(())
    }
}

pub trait Chunker {
    fn chunk(&self, data: &[u8]) -> Result<Vec<Chunk>, ChunkingError>;
}

#[derive(Error, Debug)]
pub enum ChunkingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(String),
}

pub struct FixedSizeChunker {
    pub size: usize,
}

impl FixedSizeChunker {
    pub fn new(size: usize) -> Self {
        Self { size }
    }
}

impl Chunker for FixedSizeChunker {
    fn chunk(&self, data: &[u8]) -> Result<Vec<Chunk>, ChunkingError> {
        let mut chunks = Vec::new();
        let mut offset = 0;

        for slice in data.chunks(self.size) {
            chunks.push(Chunk::new(slice.to_vec(), offset));
            offset += slice.len() as u64;
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_size_chunking() {
        let data = b"hello world, this is a test string for chunking.";
        let chunker = FixedSizeChunker::new(10);
        let chunks = chunker.chunk(data).unwrap();

        assert_eq!(chunks.len(), 5);
        assert_eq!(chunks[0].data, b"hello worl");
        assert_eq!(chunks[0].offset, 0);
        
        // simple hash verification (not full SHA256 check)
        assert!(!chunks[0].hash.is_empty());
    }
}
