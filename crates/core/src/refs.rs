//crates/core/src/refs.rs

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Reference system for SDAL
/// Manages HEAD, branches, and symbolic refs

pub struct Refs {
    repo_root: PathBuf,
}

/// Validate that a string is a valid hex hash (SHA-256)
fn is_valid_hash(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate branch name to prevent directory traversal and invalid characters
fn validate_branch_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Branch name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("Branch name cannot contain path separators");
    }
    if name.contains("..") {
        anyhow::bail!("Branch name cannot contain '..'");
    }
    if name.starts_with('.') {
        anyhow::bail!("Branch name cannot start with '.'");
    }
    Ok(())
}

impl Refs {
    pub fn new<P: AsRef<Path>>(repo_root: P) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }

    pub fn read_head(&self) -> Result<Option<String>> {
        let head_path = self.repo_root.join("HEAD");
        if !head_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&head_path)?;
        let content = content.trim();

        if content.starts_with("ref: ") {
            let ref_name = content.strip_prefix("ref: ").unwrap();
            self.read_ref(ref_name)
        } else {
            if !is_valid_hash(content) {
                anyhow::bail!("HEAD contains invalid commit hash: {}", content);
            }
            Ok(Some(content.to_string()))
        }
    }

    /// Update HEAD to point to a ref or commit
    /// Only accepts valid symbolic refs (ref: ...) or valid commit hashes
    pub fn update_head(&self, target: &str) -> Result<()> {
        if target.starts_with("ref: ") {
        } else if is_valid_hash(target) {
        } else {
            anyhow::bail!("Invalid HEAD target: must be 'ref: <path>' or a valid commit hash");
        }

        let head_path = self.repo_root.join("HEAD");
        fs::write(&head_path, target)?;
        Ok(())
    }

    pub fn read_ref(&self, name: &str) -> Result<Option<String>> {
        let ref_path = self.repo_root.join(name);
        if !ref_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&ref_path)?.trim().to_string();

        if content.is_empty() {
            return Ok(None);
        }

        if !is_valid_hash(&content) {
            anyhow::bail!("Ref '{}' contains invalid commit hash: {}", name, content);
        }

        Ok(Some(content))
    }

    pub fn update_ref(&self, name: &str, commit_hash: &str) -> Result<()> {
        if !commit_hash.is_empty() && !is_valid_hash(commit_hash) {
            anyhow::bail!("Invalid commit hash: {}", commit_hash);
        }

        let ref_path = self.repo_root.join(name);

        if let Some(parent) = ref_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&ref_path, commit_hash)?;
        Ok(())
    }

    pub fn list_branches(&self) -> Result<Vec<String>> {
        let heads_dir = self.repo_root.join("refs/heads");

        if !heads_dir.exists() {
            return Ok(vec![]);
        }

        let mut branches = Vec::new();
        for entry in fs::read_dir(heads_dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    branches.push(name.to_string());
                }
            }
        }

        Ok(branches)
    }

    /// Get the current branch name (if HEAD points to a branch)
    pub fn get_current_branch(&self) -> Result<Option<String>> {
        let head_path = self.repo_root.join("HEAD");
        if !head_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&head_path)?.trim().to_string();

        if content.starts_with("ref: ") {
            let ref_name = content.strip_prefix("ref: ").unwrap();
            if let Some(branch_name) = ref_name.strip_prefix("refs/heads/") {
                Ok(Some(branch_name.to_string()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn create_branch(&self, name: &str, commit_hash: &str) -> Result<()> {
        validate_branch_name(name)?;

        if !commit_hash.is_empty() && !is_valid_hash(commit_hash) {
            anyhow::bail!("Invalid commit hash: {}", commit_hash);
        }

        let branch_path = self.repo_root.join("refs/heads").join(name);

        if branch_path.exists() {
            anyhow::bail!("Branch '{}' already exists", name);
        }

        fs::create_dir_all(branch_path.parent().unwrap())?;
        fs::write(branch_path, commit_hash)?;

        Ok(())
    }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        validate_branch_name(name)?;

        let branch_path = self.repo_root.join("refs/heads").join(name);

        if !branch_path.exists() {
            anyhow::bail!("Branch '{}' does not exist", name);
        }

        if let Some(current) = self.get_current_branch()? {
            if current == name {
                anyhow::bail!("Cannot delete the current branch '{}'", name);
            }
        }

        fs::remove_file(branch_path)?;
        Ok(())
    }

    pub fn switch_branch(&self, name: &str) -> Result<()> {
        validate_branch_name(name)?;

        let branch_path = self.repo_root.join("refs/heads").join(name);

        if !branch_path.exists() {
            anyhow::bail!("Branch '{}' does not exist", name);
        }

        let head_path = self.repo_root.join("HEAD");
        let new_head_content = format!("ref: refs/heads/{}", name);
        fs::write(head_path, new_head_content)?;

        Ok(())
    }
}
