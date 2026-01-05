use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result;
use crate::{CheckpointIndex, Checkpoint};

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
        Ok(index)
    }
    
    /// Save checkpoint index to disk
    pub fn save_index(&self, index: &CheckpointIndex) -> Result<()> {
        fs::create_dir_all(&self.checkpoints_dir)?;
        let content = serde_json::to_string_pretty(index)?;
        fs::write(&self.index_path, content)?;
        Ok(())
    }
    
    /// Get path for a checkpoint entry
    pub fn checkpoint_entry_path(&self, id: &str) -> PathBuf {
        self.checkpoints_dir.join("entries").join(id)
    }
    
    /// Save checkpoint entry
    pub fn save_entry(&self, checkpoint: &Checkpoint) -> Result<()> {
        let entry_path = self.checkpoint_entry_path(&checkpoint.id);
        fs::create_dir_all(entry_path.parent().unwrap())?;
        let content = serde_json::to_string_pretty(checkpoint)?;
        fs::write(&entry_path, content)?;
        Ok(())
    }
    
    /// Load checkpoint entry
    pub fn load_entry(&self, id: &str) -> Result<Checkpoint> {
        let entry_path = self.checkpoint_entry_path(id);
        let content = fs::read_to_string(&entry_path)?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)?;
        Ok(checkpoint)
    }
    
    /// Delete checkpoint entry
    pub fn delete_entry(&self, id: &str) -> Result<()> {
        let entry_path = self.checkpoint_entry_path(id);
        if entry_path.exists() {
            fs::remove_file(&entry_path)?;
        }
        Ok(())
    }
    
    /// Delete all checkpoints
    pub fn delete_all(&self) -> Result<()> {
        if self.checkpoints_dir.exists() {
            fs::remove_dir_all(&self.checkpoints_dir)?;
        }
        Ok(())
    }
}
