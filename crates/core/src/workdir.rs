//crates/core/src/workdir.rs

use crate::ignore::Ignore;
use crate::index::Index;
use crate::{Blob, ChunkEntry, Object};
use anyhow::Result;
use sdal_storage::{FilesystemStorage, Storage};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Collect all files in a directory recursively, respecting ignore patterns
pub fn collect_files(
    root: &Path,
    current: &Path,
    ignore: &Ignore,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    let entries = fs::read_dir(current)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        if rel_path == ".sdal" || rel_path.starts_with(".sdal/") {
            continue;
        }

        if ignore.should_ignore(&rel_path) {
            continue;
        }

        let metadata = fs::metadata(&path)?;

        if metadata.is_file() {
            files.push((path.clone(), rel_path));
        } else if metadata.is_dir() {
            collect_files(root, &path, ignore, files)?;
        }
    }

    Ok(())
}

/// Stage all files from working directory into the index
/// This is the ONLY correct way to populate index from disk
pub fn stage_workdir(
    root: &Path,
    index: &mut Index,
    storage: &FilesystemStorage,
    ignore: &Ignore,
) -> Result<()> {
    let mut files = Vec::new();
    collect_files(root, root, ignore, &mut files)?;

    for (abs_path, rel_path) in files {
        let blob_hash = create_blob_from_file(&abs_path, storage)?;
        index.add(rel_path, blob_hash);
    }

    Ok(())
}

/// Create a blob from a file using streaming chunks
/// This is the ONLY correct way to create blobs in SDAL
pub fn create_blob_from_file(path: &Path, storage: &FilesystemStorage) -> Result<String> {
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let mut offset = 0u64;
    let mut chunks = Vec::new();
    let chunk_size = 1024 * 1024; // 1MB chunks

    loop {
        let mut buf = vec![0u8; chunk_size];
        let bytes_read = reader.read(&mut buf)?;

        if bytes_read == 0 {
            break;
        }

        buf.truncate(bytes_read);

        let mut hasher = Sha256::new();
        hasher.update(&buf);
        let chunk_hash = hex::encode(hasher.finalize());

        if let Err(e) = storage.put(&chunk_hash, &buf) {
            if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
                return Err(e.into());
            }
        }

        chunks.push(ChunkEntry {
            hash: chunk_hash,
            offset,
            size: bytes_read as u64,
        });

        offset += bytes_read as u64;
    }

    let blob = Blob {
        chunks,
        total_size: offset,
    };

    blob.validate()
        .map_err(|e| anyhow::anyhow!("Blob validation failed: {}", e))?;

    let object = Object::Blob(blob);
    let blob_json = serde_json::to_vec(&object)?;

    let mut hasher = Sha256::new();
    hasher.update(&blob_json);
    let blob_hash = hex::encode(hasher.finalize());

    if let Err(e) = storage.put(&blob_hash, &blob_json) {
        if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
            return Err(e.into());
        }
    }

    Ok(blob_hash)
}
