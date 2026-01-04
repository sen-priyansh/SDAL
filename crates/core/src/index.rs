use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Index tracks staged files for the next commit
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Index {
    /// Map of file path to blob hash
    pub entries: HashMap<String, String>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
    
    /// Load index from .sdal/index
    pub fn load<P: AsRef<Path>>(repo_root: P) -> Result<Self> {
        let index_path = repo_root.as_ref().join("index");
        
        if !index_path.exists() {
            return Ok(Self::new());
        }
        
        let content = fs::read_to_string(index_path)?;
        let index: Index = serde_json::from_str(&content)?;
        Ok(index)
    }
    
    /// Save index to .sdal/index
    pub fn save<P: AsRef<Path>>(&self, repo_root: P) -> Result<()> {
        let index_path = repo_root.as_ref().join("index");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(index_path, content)?;
        Ok(())
    }
    
    /// Add a file to the index
    pub fn add(&mut self, path: String, blob_hash: String) {
        self.entries.insert(path, blob_hash);
    }
    
    /// Remove a file from the index
    pub fn remove(&mut self, path: &str) {
        self.entries.remove(path);
    }
    
    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    
    /// Check if a path is staged
    pub fn is_staged(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }
    
    /// Get blob hash for a path
    pub fn get(&self, path: &str) -> Option<&String> {
        self.entries.get(path)
    }
}
