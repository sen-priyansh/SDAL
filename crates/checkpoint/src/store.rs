use crate::{Checkpoint, CheckpointIndex};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Validate checkpoint ID to prevent path traversal
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() {
        anyhow::bail!("Checkpoint ID cannot be empty");
    }
    if id.contains('/') || id.contains('\\') {
        anyhow::bail!("Checkpoint ID cannot contain path separators: {}", id);
    }
    if id.contains("..") {
        anyhow::bail!("Checkpoint ID cannot contain '..': {}", id);
    }
    Ok(())
}

/// Atomic write to prevent corruption on crash
fn atomic_write(path: &Path, data: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Checkpoint storage manager
pub struct CheckpointStore {
    checkpoints_dir: PathBuf,
    index_path: PathBuf,
}

impl CheckpointStore {
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Self {
        let checkpoints_dir = repo_root.as_ref().join("checkpoints");
        let index_path = checkpoints_dir.join("index");
        Self {
            checkpoints_dir,
            index_path,
        }
    }

    /// Load checkpoint index from disk
    pub fn load_index(&self) -> Result<CheckpointIndex> {
        if !self.index_path.exists() {
            return Ok(CheckpointIndex::new());
        }

        let content = fs::read_to_string(&self.index_path)?;
        let index: CheckpointIndex = serde_json::from_str(&content)?;

        // Validate after deserialization
        validate_index(&index)?;

        Ok(index)
    }

    /// Save checkpoint index to disk
    pub fn save_index(&self, index: &CheckpointIndex) -> Result<()> {
        fs::create_dir_all(&self.checkpoints_dir)?;

        // Validate before saving
        validate_index(index)?;

        let content = serde_json::to_string_pretty(index)?;
        atomic_write(&self.index_path, &content)?;
        Ok(())
    }

    /// Get path for a checkpoint entry
    pub fn checkpoint_entry_path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self.checkpoints_dir.join("entries").join(id))
    }

    /// Save checkpoint entry
    pub fn save_entry(&self, checkpoint: &Checkpoint) -> Result<()> {
        // Validate checkpoint
        validate_checkpoint(checkpoint)?;

        let entry_path = self.checkpoint_entry_path(&checkpoint.id)?;
        fs::create_dir_all(entry_path.parent().unwrap())?;

        let content = serde_json::to_string_pretty(checkpoint)?;
        atomic_write(&entry_path, &content)?;
        Ok(())
    }

    /// Load checkpoint entry
    pub fn load_entry(&self, id: &str) -> Result<Checkpoint> {
        let entry_path = self.checkpoint_entry_path(id)?;
        let content = fs::read_to_string(&entry_path)?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)?;

        // Validate after deserialization
        validate_checkpoint(&checkpoint)?;

        Ok(checkpoint)
    }

    /// Delete checkpoint entry
    pub fn delete_entry(&self, id: &str) -> Result<()> {
        let entry_path = self.checkpoint_entry_path(id)?;
        if entry_path.exists() {
            fs::remove_file(&entry_path)?;
        }
        Ok(())
    }

    /// Delete all checkpoints (safer: removes contents but keeps structure)
    pub fn delete_all(&self) -> Result<()> {
        if !self.checkpoints_dir.exists() {
            return Ok(());
        }

        // Remove entries directory
        let entries_dir = self.checkpoints_dir.join("entries");
        if entries_dir.exists() {
            fs::remove_dir_all(&entries_dir)?;
        }

        // Reset index to empty
        let empty_index = CheckpointIndex::new();
        self.save_index(&empty_index)?;

        Ok(())
    }
}

/// Validate checkpoint index
fn validate_index(index: &CheckpointIndex) -> Result<()> {
    // Check all checkpoint IDs are valid
    for checkpoint in &index.checkpoints {
        validate_id(&checkpoint.id)?;
        validate_checkpoint(checkpoint)?;
    }

    // Check current points to existing checkpoint if set
    if let Some(current_id) = &index.current {
        if !index.checkpoints.iter().any(|cp| &cp.id == current_id) {
            anyhow::bail!("Current checkpoint '{}' not found in index", current_id);
        }
    }

    Ok(())
}

/// Validate checkpoint
fn validate_checkpoint(checkpoint: &Checkpoint) -> Result<()> {
    // ID must be valid
    validate_id(&checkpoint.id)?;

    // Tree root must be non-empty
    if checkpoint.tree_root.is_empty() {
        anyhow::bail!("Checkpoint tree_root cannot be empty");
    }

    // Tree root must be valid hash (64 hex chars)
    if checkpoint.tree_root.len() != 64
        || !checkpoint.tree_root.chars().all(|c| c.is_ascii_hexdigit())
    {
        anyhow::bail!("Checkpoint tree_root must be a valid SHA-256 hash");
    }

    // Parent commit must be valid hash if present
    if let Some(ref parent) = checkpoint.parent_commit {
        if parent.len() != 64 || !parent.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("Checkpoint parent_commit must be a valid SHA-256 hash");
        }
    }

    // Timestamp should be reasonable (not negative, not too far in future)
    if checkpoint.timestamp < 0 {
        anyhow::bail!("Checkpoint timestamp cannot be negative");
    }

    // Sanity check: not more than 1 year in the future
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if checkpoint.timestamp > now + (365 * 24 * 60 * 60) {
        anyhow::bail!("Checkpoint timestamp is too far in the future");
    }

    Ok(())
}
