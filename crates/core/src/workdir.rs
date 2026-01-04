use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context};
use crate::{Tree, TreeEntry, Object, Blob};
use sdal_storage::{Storage, FilesystemStorage};
use sdal_chunking::{Chunker, FixedSizeChunker};
use sha2::{Digest, Sha256};

/// Build a Tree object from a directory on disk
pub fn build_tree_from_dir<P: AsRef<Path>>(
    dir: P,
    storage: &FilesystemStorage,
) -> Result<String> {
    let dir = dir.as_ref();
    let mut tree = Tree::new();
    
    let entries = fs::read_dir(dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        
        // Skip .sdal directory
        if name == ".sdal" {
            continue;
        }
        
        let metadata = fs::metadata(&path)?;
        
        if metadata.is_file() {
            // Create blob for file
            let data = fs::read(&path)?;
            let chunker = FixedSizeChunker::new(1024 * 1024);
            let chunks = chunker.chunk(&data)?;
            
            let mut chunk_entries = Vec::new();
            for chunk in chunks {
                storage.put(&chunk.hash, &chunk.data)?;
                chunk_entries.push(crate::ChunkEntry::from(&chunk));
            }
            
            let blob = Blob {
                chunks: chunk_entries,
                total_size: data.len() as u64,
            };
            blob.validate().map_err(|e| anyhow::anyhow!("Blob validation failed: {}", e))?;
            
            let object = Object::Blob(blob);
            let blob_json = serde_json::to_vec(&object)?;
            
            let mut hasher = Sha256::new();
            hasher.update(&blob_json);
            let blob_hash = hex::encode(hasher.finalize());
            
            storage.put(&blob_hash, &blob_json)?;
            
            tree.add_entry(
                name,
                TreeEntry::Blob {
                    hash: blob_hash,
                    size: metadata.len(),
                },
            );
        } else if metadata.is_dir() {
            // Recursively build tree for subdirectory
            let subtree_hash = build_tree_from_dir(&path, storage)?;
            tree.add_entry(name, TreeEntry::Tree { hash: subtree_hash });
        }
    }
    
    // Store the tree itself
    tree.validate().map_err(|e| anyhow::anyhow!("Tree validation failed: {}", e))?;
    let tree_object = Object::Tree(tree);
    let tree_json = serde_json::to_vec(&tree_object)?;
    
    let mut hasher = Sha256::new();
    hasher.update(&tree_json);
    let tree_hash = hex::encode(hasher.finalize());
    
    storage.put(&tree_hash, &tree_json)?;
    
    Ok(tree_hash)
}
