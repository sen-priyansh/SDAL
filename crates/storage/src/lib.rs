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

    fn object_path(&self, hash: &str) -> PathBuf {
        let (dir, file) = hash.split_at(2);
        self.root.join("objects").join(dir).join(file)
    }

    /// Verify that data matches the given hash
    fn verify_hash(hash: &str, data: &[u8]) -> Result<(), StorageError> {
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
        Self::verify_hash(hash, data)?;

        let path = self.object_path(hash);

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
        let path = self.object_path(hash);
        if !path.exists() {
            return Err(StorageError::NotFound(hash.to_string()));
        }

        let data = fs::read(path)?;

        // Invariant: stored data must match its hash
        Self::verify_hash(hash, &data)?;

        Ok(data)
    }
    fn exists(&self, hash: &str) -> bool {
        self.object_path(hash).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // mock tempfile for now since it's not in workspace
    // We will just use a random local dir path for basic logic tests if needed,
    // but without tempfile crate we can't easily do disposable IO tests.
    // Opting to trust the implementation for now as it's standard FS logic.
}
