//crates/core/src/checkout.rs

use crate::{Object, TreeEntry};
use anyhow::Result;
use sdal_storage::{FilesystemStorage, Storage};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Restore files from a tree object to the working directory
///
/// # Invariants
/// - Caller must ensure the working directory is clean or use appropriate safety checks
/// - This function will overwrite existing files without warning
/// - Files not present in the tree will NOT be removed (caller's responsibility)
pub fn restore_tree(tree_hash: &str, storage: &FilesystemStorage, target_dir: &Path) -> Result<()> {
    restore_tree_recursive(tree_hash, storage, target_dir, "")
}

/// Restore files from a tree and remove files that don't exist in the tree,
/// while preserving any files/dirs that match the .sdalignore rules.
///
/// This is the "clean" version used for `reset --hard` and `checkout`.
pub fn restore_tree_clean(
    tree_hash: &str,
    storage: &FilesystemStorage,
    target_dir: &Path,
    ignore: &crate::ignore::Ignore,
) -> Result<()> {
    let mut expected_paths = HashSet::new();
    collect_tree_paths(tree_hash, storage, "", &mut expected_paths)?;

    restore_tree(tree_hash, storage, target_dir)?;

    remove_unexpected_files(target_dir, target_dir, &expected_paths, ignore)?;

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

/// Remove files and directories that are not in the expected set,
/// skipping anything that is ignored by .sdalignore.
fn remove_unexpected_files(
    root_dir: &Path,
    current_dir: &Path,
    expected_paths: &HashSet<String>,
    ignore: &crate::ignore::Ignore,
) -> Result<()> {
    if current_dir.file_name().and_then(|n| n.to_str()) == Some(".sdal") {
        return Ok(());
    }

    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        let filename = path.file_name().and_then(|n| n.to_str());

        if matches!(filename, Some(".sdal") | Some(".sdalignore")) {
            continue;
        }

        let rel_path = path
            .strip_prefix(root_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        // Never delete ignored files/directories (target/, node_modules/, etc.)
        if ignore.should_ignore(&rel_path) {
            continue;
        }

        // Use lstat (no symlink following) so a clean checkout never traverses
        // into, or deletes through, a symlink whose target may live outside the
        // repository.
        let ftype = fs::symlink_metadata(&path)?.file_type();

        if ftype.is_symlink() {
            // An untracked symlink: remove the link itself, never its target.
            if !expected_paths.contains(&rel_path) {
                fs::remove_file(&path)?;
            }
        } else if ftype.is_dir() {
            remove_unexpected_files(root_dir, &path, expected_paths, ignore)?;

            if !expected_paths.contains(&rel_path) {
                if let Ok(mut entries) = fs::read_dir(&path) {
                    if entries.next().is_none() {
                        fs::remove_dir(&path)?;
                    }
                }
            }
        } else if ftype.is_file() {
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn clean_checkout_does_not_delete_through_symlink_outside_repo() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("sdal_co_{}_{}", std::process::id(), nanos));
        let external = base.join("external");
        let work = base.join("work");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&work).unwrap();
        let keep = external.join("keep.txt");
        fs::write(&keep, b"precious external data").unwrap();
        // an untracked symlink inside the working tree pointing OUTSIDE the repo
        symlink(&external, work.join("link")).unwrap();

        let expected: HashSet<String> = HashSet::new();
        let ignore = crate::ignore::Ignore::load(&work);
        let _ = remove_unexpected_files(&work, &work, &expected, &ignore);

        assert!(
            keep.exists(),
            "clean checkout must not delete files through a symlink that points outside the repo"
        );
        let _ = fs::remove_dir_all(&base);
    }
}
