use crate::{store::CheckpointStore, Checkpoint, CheckpointIndex};
use anyhow::Result;
use sdal_core::{checkout, refs::Refs, workdir};
use sdal_storage::{FilesystemStorage, Storage};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Save current working directory as a checkpoint
pub fn save_checkpoint(
    repo_root: &Path,
    working_dir: &Path,
    message: Option<String>,
) -> Result<String> {
    let store = CheckpointStore::new(repo_root);
    let mut index = store.load_index()?;

    // Get current HEAD (may be None if no commits yet)
    let refs = Refs::new(repo_root);
    let parent_commit = refs.read_head()?;

    // Build tree from working directory using proper workflow
    let storage = FilesystemStorage::new(repo_root)?;
    let ignore = sdal_core::ignore::Ignore::load(working_dir);

    // Stage working directory into temporary index
    let mut temp_index = sdal_core::index::Index::new();
    sdal_core::workdir::stage_workdir(working_dir, &mut temp_index, &storage, &ignore)?;

    // Build tree from index
    use sdal_core::{Object, Tree, TreeEntry};
    use std::collections::HashMap;

    let mut entries_map: HashMap<String, String> = HashMap::new();
    for (path, hash) in &temp_index.entries {
        entries_map.insert(path.clone(), hash.clone());
    }

    // Use build_tree_recursive (this function should be in main.rs, but we'll inline it for now)
    fn build_tree_recursive(
        entries: &HashMap<String, String>,
        storage: &FilesystemStorage,
    ) -> Result<String> {
        let mut tree = Tree::new();
        let mut subfolders: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut files_at_this_level = std::collections::HashSet::new();

        for (path, hash) in entries {
            if let Some(pos) = path.find('/') {
                let (dir_name, remaining) = path.split_at(pos);
                let remaining = &remaining[1..];

                if files_at_this_level.contains(dir_name) {
                    anyhow::bail!("File/directory collision: '{}'", dir_name);
                }

                subfolders
                    .entry(dir_name.to_string())
                    .or_default()
                    .insert(remaining.to_string(), hash.clone());
            } else {
                if subfolders.contains_key(path.as_str()) {
                    anyhow::bail!("File/directory collision: '{}'", path);
                }

                files_at_this_level.insert(path.clone());
                tree.add_entry(
                    path.clone(),
                    TreeEntry::Blob {
                        hash: hash.clone(),
                        size: 0,
                    },
                );
            }
        }

        for (dir_name, dir_entries) in subfolders {
            let subtree_hash = build_tree_recursive(&dir_entries, storage)?;
            tree.add_entry(dir_name, TreeEntry::Tree { hash: subtree_hash });
        }

        tree.sort();
        tree.validate()
            .map_err(|e| anyhow::anyhow!("Tree validation failed: {}", e))?;

        let tree_object = Object::Tree(tree);
        let tree_json = serde_json::to_vec(&tree_object)?;

        let mut hasher = Sha256::new();
        hasher.update(&tree_json);
        let tree_hash = hex::encode(hasher.finalize());

        if let Err(e) = storage.put(&tree_hash, &tree_json) {
            if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
                return Err(e.into());
            }
        }
        Ok(tree_hash)
    }

    let tree_hash = build_tree_recursive(&entries_map, &storage)?;

    // Create checkpoint
    let id = index.next_id();
    let checkpoint = Checkpoint {
        id: id.clone(),
        tree_root: tree_hash,
        parent_commit,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64,
        message,
    };

    // Save entry and update index
    store.save_entry(&checkpoint)?;
    index.add(checkpoint);
    store.save_index(&index)?;

    Ok(id)
}

/// List all checkpoints
pub fn list_checkpoints(repo_root: &Path) -> Result<CheckpointIndex> {
    let store = CheckpointStore::new(repo_root);
    store.load_index()
}

/// Checkout to a specific checkpoint
pub fn checkout_checkpoint(
    repo_root: &Path,
    working_dir: &Path,
    checkpoint_id: &str,
) -> Result<()> {
    let store = CheckpointStore::new(repo_root);
    let mut index = store.load_index()?;

    // Find checkpoint
    let checkpoint = index
        .find(checkpoint_id)
        .ok_or(anyhow::anyhow!("Checkpoint '{}' not found", checkpoint_id))?
        .clone();

    // Restore working directory from checkpoint tree
    let storage = FilesystemStorage::new(repo_root)?;
    checkout::restore_tree(&checkpoint.tree_root, &storage, working_dir)?;

    // Update current pointer
    index.current = Some(checkpoint_id.to_string());
    store.save_index(&index)?;

    Ok(())
}

/// Drop (delete) a specific checkpoint
pub fn drop_checkpoint(repo_root: &Path, checkpoint_id: &str) -> Result<()> {
    let store = CheckpointStore::new(repo_root);
    let mut index = store.load_index()?;

    if !index.remove(checkpoint_id) {
        anyhow::bail!("Checkpoint '{}' not found", checkpoint_id);
    }

    store.delete_entry(checkpoint_id)?;
    store.save_index(&index)?;

    Ok(())
}

/// Delete all checkpoints (called on commit)
pub fn clear_all_checkpoints(repo_root: &Path) -> Result<()> {
    let store = CheckpointStore::new(repo_root);
    store.delete_all()
}

/// Get current checkpoint tree hash (if any) for commit
pub fn get_current_tree(repo_root: &Path) -> Result<Option<String>> {
    let store = CheckpointStore::new(repo_root);
    let index = store.load_index()?;

    if let Some(current_id) = &index.current {
        if let Some(checkpoint) = index.find(current_id) {
            return Ok(Some(checkpoint.tree_root.clone()));
        }
    }

    Ok(None)
}
