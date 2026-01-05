use std::path::Path;
use anyhow::Result;
use crate::{Checkpoint, CheckpointIndex, store::CheckpointStore};
use sdal_core::{refs::Refs, workdir, checkout};
use sdal_storage::FilesystemStorage;

/// Save current working directory as a checkpoint
pub fn save_checkpoint(
    repo_root: &Path,
    working_dir: &Path,
    message: Option<String>,
) -> Result<String> {
    let store = CheckpointStore::new(repo_root);
    let mut index = store.load_index()?;
    
    // Get current HEAD
    let refs = Refs::new(repo_root);
    let parent_commit = refs.read_head()?
        .ok_or(anyhow::anyhow!("No commits yet - create an initial commit first"))?;
    
    // Build tree from working directory
    let storage = FilesystemStorage::new(repo_root)?;
    let tree_hash = workdir::build_tree_from_dir(working_dir, &storage)?;
    
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
    let checkpoint = index.find(checkpoint_id)
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
