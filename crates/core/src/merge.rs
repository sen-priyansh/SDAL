use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::fs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::{Object, Commit, Tree, TreeEntry};
use sdal_storage::{FilesystemStorage, Storage};

/// Merge state stored in .sdal/MERGE_STATE
#[derive(Serialize, Deserialize, Debug)]
pub struct MergeState {
    pub ours: String,
    pub theirs: String,
    pub target_branch: String,
    pub conflicts: Vec<String>,
}

impl MergeState {
    pub fn load(repo_root: &Path) -> Result<Option<Self>> {
        let merge_state_path = repo_root.join("MERGE_STATE");
        if !merge_state_path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(merge_state_path)?;
        let state: MergeState = serde_json::from_str(&content)?;
        Ok(Some(state))
    }
    
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let merge_state_path = repo_root.join("MERGE_STATE");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(merge_state_path, content)?;
        Ok(())
    }
    
    pub fn delete(repo_root: &Path) -> Result<()> {
        let merge_state_path = repo_root.join("MERGE_STATE");
        if merge_state_path.exists() {
            fs::remove_file(merge_state_path)?;
        }
        Ok(())
    }
}

/// Find the merge base (lowest common ancestor) of two commits
pub fn find_merge_base(
    ours_hash: &str,
    theirs_hash: &str,
    storage: &FilesystemStorage,
) -> Result<String> {
    // Collect all ancestors of ours
    let mut ours_ancestors = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(ours_hash.to_string());
    
    while let Some(hash) = queue.pop_front() {
        if ours_ancestors.contains(&hash) {
            continue;
        }
        ours_ancestors.insert(hash.clone());
        
        // Load commit and add parents
        let commit_data = storage.get(&hash)?;
        let obj: Object = serde_json::from_slice(&commit_data)?;
        if let Object::Commit(commit) = obj {
            for parent in &commit.parents {
                queue.push_back(parent.clone());
            }
        }
    }
    
    // BFS from theirs, find first common ancestor
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(theirs_hash.to_string());
    
    while let Some(hash) = queue.pop_front() {
        if visited.contains(&hash) {
            continue;
        }
        visited.insert(hash.clone());
        
        // Check if this is in ours ancestors
        if ours_ancestors.contains(&hash) {
            return Ok(hash);
        }
        
        // Load commit and add parents
        let commit_data = storage.get(&hash)?;
        let obj: Object = serde_json::from_slice(&commit_data)?;
        if let Object::Commit(commit) = obj {
            for parent in &commit.parents {
                queue.push_back(parent.clone());
            }
        }
    }
    
    anyhow::bail!("No common ancestor found")
}

/// Convert tree to flat map of paths -> hashes
fn flatten_tree(tree_hash: &str, storage: &FilesystemStorage) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    
    let tree_data = storage.get(tree_hash)?;
    let obj: Object = serde_json::from_slice(&tree_data)?;
    
    if let Object::Tree(tree) = obj {
        for (name, entry) in tree.entries {
            match entry {
                TreeEntry::Blob { hash, .. } => {
                    result.insert(name, hash);
                }
                TreeEntry::Tree { .. } => {
                    // For now, treat nested trees as single entries
                    // Full implementation would recursively flatten
                }
            }
        }
    }
    
    Ok(result)
}

/// Perform 3-way merge on trees
/// Returns (merged_tree, conflicts)
pub fn merge_trees(
    base_tree_hash: &str,
    ours_tree_hash: &str,
    theirs_tree_hash: &str,
    storage: &FilesystemStorage,
) -> Result<(Tree, Vec<String>)> {
    let base_files = flatten_tree(base_tree_hash, storage)?;
    let ours_files = flatten_tree(ours_tree_hash, storage)?;
    let theirs_files = flatten_tree(theirs_tree_hash, storage)?;
    
    // Collect all paths
    let mut all_paths = HashSet::new();
    all_paths.extend(base_files.keys().cloned());
    all_paths.extend(ours_files.keys().cloned());
    all_paths.extend(theirs_files.keys().cloned());
    
    let mut merged_tree = Tree::new();
    let mut conflicts = Vec::new();
    
    for path in all_paths {
        let base = base_files.get(&path);
        let ours = ours_files.get(&path);
        let theirs = theirs_files.get(&path);
        
        // 3-way merge logic
        let result_hash = if ours == theirs {
            // Both sides agree
            ours.cloned()
        } else if base == ours {
            // We didn't change it, they did
            theirs.cloned()
        } else if base == theirs {
            // They didn't change it, we did
           ours.cloned()
        } else {
            // Conflict: both changed differently
            conflicts.push(path.clone());
            None
        };
        
        if let Some(hash) = result_hash {
            merged_tree.add_entry(
                path.clone(),
                TreeEntry::Blob {
                    hash,
                    size: 0, // Size tracking can be added later
                },
            );
        }
    }
    
    Ok((merged_tree, conflicts))
}

/// Perform merge operation
pub fn perform_merge(
    target_branch: &str,
    ours_hash: &str,
    repo_root: &Path,
    storage: &FilesystemStorage,
) -> Result<MergeState> {
    // Get theirs commit from target branch
    let branch_ref = format!("refs/heads/{}", target_branch);
    let branch_path = repo_root.join(&branch_ref);
    
    if !branch_path.exists() {
        anyhow::bail!("Branch '{}' does not exist", target_branch);
    }
    
    let theirs_hash = fs::read_to_string(&branch_path)?.trim().to_string();
    
    // Find merge base
    let base_hash = find_merge_base(ours_hash, &theirs_hash, storage)?;
    
    // Load commits
    let base_commit = load_commit(&base_hash, storage)?;
    let ours_commit = load_commit(ours_hash, storage)?;
    let theirs_commit = load_commit(&theirs_hash, storage)?;
    
    // Merge trees
    let (_merged_tree, conflicts) = merge_trees(
        &base_commit.tree,
        &ours_commit.tree,
        &theirs_commit.tree,
        storage,
    )?;
    
    // Create merge state
    let merge_state = MergeState {
        ours: ours_hash.to_string(),
        theirs: theirs_hash,
        target_branch: target_branch.to_string(),
        conflicts,
    };
    
    Ok(merge_state)
}

fn load_commit(hash: &str, storage: &FilesystemStorage) -> Result<Commit> {
    let data = storage.get(hash)?;
    let obj: Object = serde_json::from_slice(&data)?;
    if let Object::Commit(commit) = obj {
        Ok(commit)
    } else {
        anyhow::bail!("Not a commit object")
    }
}
