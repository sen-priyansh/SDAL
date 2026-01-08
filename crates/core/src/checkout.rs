//crates/core/src/checkout.rs

use crate::{Object, TreeEntry};
use anyhow::Result;
use sdal_storage::{FilesystemStorage, Storage};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Restore files from a tree object to the working directory
///
/// # Invariants
/// - Caller must ensure the working directory is clean or use appropriate safety checks
/// - This function will overwrite existing files without warning
/// - Files not present in the tree will NOT be removed (caller's responsibility)
pub fn restore_tree(tree_hash: &str, storage: &FilesystemStorage, target_dir: &Path) -> Result<()> {
    restore_tree_recursive(tree_hash, storage, target_dir, "")
}

/// Restore files from a tree and remove files that don't exist in the tree
///
/// This is the "clean" version that ensures the working directory matches the tree exactly.
/// Use this for operations like `reset --hard` and `checkout`.
pub fn restore_tree_clean(
    tree_hash: &str,
    storage: &FilesystemStorage,
    target_dir: &Path,
) -> Result<()> {
    // 1. Collect all paths that SHOULD exist in the tree
    let mut expected_paths = HashSet::new();
    collect_tree_paths(tree_hash, storage, "", &mut expected_paths)?;

    // 2. Restore the tree
    restore_tree(tree_hash, storage, target_dir)?;

    // 3. Remove files and directories that shouldn't exist
    remove_unexpected_files(target_dir, target_dir, &expected_paths)?;

    Ok(())
}

/// Internal recursive function for restoring a tree
fn restore_tree_recursive(
    tree_hash: &str,
    storage: &FilesystemStorage,
    target_dir: &Path,
    prefix: &str,
) -> Result<()> {
    let tree_data = storage.get(tree_hash)?;
    let object = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

    if let Object::Tree(tree) = object {
        // Validate tree structure (optional but good for corruption defense)
        tree.validate()
            .map_err(|e| anyhow::anyhow!("Tree validation failed during checkout: {}", e))?;
        for (name, entry) in tree.entries {
            let file_path = target_dir.join(&name);

            match entry {
                TreeEntry::Blob { hash, .. } => {
                    restore_blob(&hash, storage, &file_path)?;
                }
                TreeEntry::Tree { hash } => {
                    fs::create_dir_all(&file_path)?;
                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", prefix, name)
                    };
                    restore_tree_recursive(&hash, storage, &file_path, &new_prefix)?;
                }
            }
        }
    }

    Ok(())
}

/// Collect all file paths that exist in a tree (recursively)
fn collect_tree_paths(
    tree_hash: &str,
    storage: &FilesystemStorage,
    prefix: &str,
    paths: &mut HashSet<String>,
) -> Result<()> {
    let tree_data = storage.get(tree_hash)?;
    let object = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

    if let Object::Tree(tree) = object {
        for (name, entry) in tree.entries {
            let full_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };

            match entry {
                TreeEntry::Blob { .. } => {
                    paths.insert(full_path);
                }
                TreeEntry::Tree { hash } => {
                    // Add directory itself
                    paths.insert(full_path.clone());
                    // Recurse into subdirectory
                    collect_tree_paths(&hash, storage, &full_path, paths)?;
                }
            }
        }
    }

    Ok(())
}

/// Remove files and directories that are not in the expected set
fn remove_unexpected_files(
    root_dir: &Path,
    current_dir: &Path,
    expected_paths: &HashSet<String>,
) -> Result<()> {
    // Don't remove .sdal directory
    if current_dir.file_name().and_then(|n| n.to_str()) == Some(".sdal") {
        return Ok(());
    }

    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // Directory doesn't exist, nothing to clean
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Get filename for checking
        let filename = path.file_name().and_then(|n| n.to_str());

        // Skip protected files and directories
        if matches!(filename, Some(".sdal") | Some(".sdalignore")) {
            continue;
        }

        // Get relative path from root
        let rel_path = path
            .strip_prefix(root_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        if path.is_dir() {
            // Recurse into directory first
            remove_unexpected_files(root_dir, &path, expected_paths)?;

            // Remove directory if it's not expected and is now empty
            if !expected_paths.contains(&rel_path) {
                if let Ok(mut entries) = fs::read_dir(&path) {
                    if entries.next().is_none() {
                        // Directory is empty, remove it
                        fs::remove_dir(&path)?;
                    }
                }
            }
        } else if path.is_file() {
            // Remove file if it's not expected
            if !expected_paths.contains(&rel_path) {
                fs::remove_file(&path)?;
            }
        }
    }

    Ok(())
}

/// Restore a single blob to a file path using streaming writes
///
/// This function writes chunks directly to disk without buffering the entire file in memory,
/// making it memory-safe for large files.
pub fn restore_blob(
    blob_hash: &str,
    storage: &FilesystemStorage,
    target_path: &Path,
) -> Result<()> {
    let blob_data = storage.get(blob_hash)?;
    let object = Object::from_bytes(&blob_data).map_err(anyhow::Error::msg)?;

    if let Object::Blob(blob) = object {
        // Validate blob structure (optional but good for corruption defense)
        blob.validate()
            .map_err(|e| anyhow::anyhow!("Blob validation failed during checkout: {}", e))?;
        // Create parent directories if needed
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Stream-write chunks directly to file (memory-safe for large files)
        if target_path.exists() {
            std::fs::remove_file(target_path)?;
        }
        let mut file = std::fs::File::create(target_path)?;
        for chunk_entry in blob.chunks {
            let chunk_data = storage.get(&chunk_entry.hash)?;
            file.write_all(&chunk_data)?;
        }
        file.sync_all()?;
    }

    Ok(())
}
