//crates/storage/src/lib.rs

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Corruption detected: {0}")]
    Corruption(String),
}

pub trait Storage {
    fn put(&self, hash: &str, data: &[u8]) -> Result<(), StorageError>;
    fn get(&self, hash: &str) -> Result<Vec<u8>, StorageError>;
    fn exists(&self, hash: &str) -> bool;

    /// Stream data from storage into a writer
    /// This is the preferred method for large objects
    fn get_stream<W: std::io::Write>(&self, hash: &str, writer: &mut W)
    -> Result<(), StorageError>;
}

pub struct FilesystemStorage {
    root: PathBuf,
}

impl FilesystemStorage {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;

        // Explicitly create objects directory
        fs::create_dir_all(root.join("objects"))?;

        Ok(Self { root })
    }

    /// Validate hash format to prevent path traversal
    fn validate_hash(hash: &str) -> Result<(), StorageError> {
        if hash.len() != 64 {
            return Err(StorageError::Corruption(format!(
                "Invalid hash length: expected 64, got {}",
                hash.len()
            )));
        }
        if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StorageError::Corruption(format!(
                "Invalid hash format: must be hexadecimal"
            )));
        }
        Ok(())
    }

    fn object_path(&self, hash: &str) -> Result<PathBuf, StorageError> {
        Self::validate_hash(hash)?;
        let (dir, file) = hash.split_at(2);
        Ok(self.root.join("objects").join(dir).join(file))
    }

    /// Verify that data matches the given hash
    fn verify_data_hash(hash: &str, data: &[u8]) -> Result<(), StorageError> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let computed = hex::encode(hasher.finalize());

        if computed != hash {
            return Err(StorageError::Corruption(format!(
                "Hash mismatch: expected {}, computed {}",
                hash, computed
            )));
        }

        Ok(())
    }
}

impl Storage for FilesystemStorage {
    fn put(&self, hash: &str, data: &[u8]) -> Result<(), StorageError> {
        // Invariant: hash must match data
        Self::verify_data_hash(hash, data)?;

        let path = self.object_path(hash)?;

        // Prevent overwrites - objects are immutable
        if path.exists() {
            return Err(StorageError::AlreadyExists(hash.to_string()));
        }

        // Create parent directory
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Atomic write: write to temp file, then rename
        // This ensures we never have partial writes on crash
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, data)?;
        fs::rename(&tmp_path, &path)?;

        Ok(())
    }

    fn get(&self, hash: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.object_path(hash)?;
        if !path.exists() {
            return Err(StorageError::NotFound(hash.to_string()));
        }

        // Use streaming read with in-memory buffer
        let mut data = Vec::new();
        self.get_stream(hash, &mut data)?;

        Ok(data)
    }

    fn get_stream<W: std::io::Write>(
        &self,
        hash: &str,
        writer: &mut W,
    ) -> Result<(), StorageError> {
        use std::io::Read;

        let path = self.object_path(hash)?;
        if !path.exists() {
            return Err(StorageError::NotFound(hash.to_string()));
        }

        let file = fs::File::open(&path)?;
        let mut reader = std::io::BufReader::new(file);

        // Stream data in chunks with hash verification
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];
            hasher.update(chunk);
            writer.write_all(chunk)?;
        }

        // Verify hash after streaming
        let computed = hex::encode(hasher.finalize());
        if computed != hash {
            return Err(StorageError::Corruption(format!(
                "Hash mismatch: expected {}, computed {}",
                hash, computed
            )));
        }

        Ok(())
    }

    fn exists(&self, hash: &str) -> bool {
        // Validate hash before checking existence
        if Self::validate_hash(hash).is_err() {
            return false;
        }

        // Safe to unwrap because we just validated
        self.object_path(hash).unwrap().exists()
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here
    // Requires tempfile crate for proper testing
}
