use std::path::Path;
use std::fs;
use anyhow::Result;
use crate::{Tree, TreeEntry, Object, Blob};
use sdal_storage::{Storage, FilesystemStorage};

/// Restore files from a tree object to the working directory
pub fn restore_tree(
    tree_hash: &str,
    storage: &FilesystemStorage,
    target_dir: &Path,
) -> Result<()> {
    let tree_data = storage.get(tree_hash)?;
    let object: Object = serde_json::from_slice(&tree_data)?;
    
    if let Object::Tree(tree) = object {
        for (name, entry) in tree.entries {
            let file_path = target_dir.join(&name);
            
            match entry {
                TreeEntry::Blob { hash, .. } => {
                    restore_blob(&hash, storage, &file_path)?;
                }
                TreeEntry::Tree { hash } => {
                    fs::create_dir_all(&file_path)?;
                    restore_tree(&hash, storage, &file_path)?;
                }
            }
        }
    }
    
    Ok(())
}

/// Restore a single blob to a file path
pub fn restore_blob(
    blob_hash: &str,
    storage: &FilesystemStorage,
    target_path: &Path,
) -> Result<()> {
    let blob_data = storage.get(blob_hash)?;
    let object: Object = serde_json::from_slice(&blob_data)?;
    
    if let Object::Blob(blob) = object {
        // Create parent directories if needed
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Reconstruct file from chunks
        let mut file_content = Vec::new();
        for chunk_entry in blob.chunks {
            let chunk_data = storage.get(&chunk_entry.hash)?;
            file_content.extend_from_slice(&chunk_data);
        }
        
        fs::write(target_path, file_content)?;
    }
    
    Ok(())
}
