//crates/core/src/merge.rs

use crate::index::Index;
use crate::{Commit, Object, Tree, TreeEntry};
use anyhow::Result;
use sdal_storage::{FilesystemStorage, Storage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

/// Merge state stored in .sdal/MERGE_STATE
#[derive(Serialize, Deserialize, Debug)]
pub struct MergeState {
    pub ours: String,
    pub theirs: String,
    pub target_branch: String,
    pub conflicts: Vec<String>,
    pub merged_tree_hash: String, // Hash of the merged tree
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

        let commit_data = storage.get(&hash)?;
        let obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;
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

        if ours_ancestors.contains(&hash) {
            return Ok(hash);
        }

        let commit_data = storage.get(&hash)?;
        let obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;
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
    let obj = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

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

/// Populate index from a tree (recursively)
pub fn populate_index_from_tree(
    tree_hash: &str,
    storage: &FilesystemStorage,
    index: &mut Index,
    prefix: &str,
) -> Result<()> {
    let tree_data = storage.get(tree_hash)?;
    let obj = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

    if let Object::Tree(tree) = obj {
        for (name, entry) in tree.entries {
            let full_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };

            match entry {
                TreeEntry::Blob { hash, .. } => {
                    index.add(full_path, hash);
                }
                TreeEntry::Tree { hash } => {
                    populate_index_from_tree(&hash, storage, index, &full_path)?;
                }
            }
        }
    }

    Ok(())
}

/// Write conflict files (.ours and .theirs) for a given path
pub fn write_conflict_files(
    path: &str,
    ours_hash: &str,
    theirs_hash: &str,
    storage: &FilesystemStorage,
    working_dir: &Path,
) -> Result<()> {
    use std::io::Write;

    let ours_path = working_dir.join(format!("{}.ours", path));
    if let Some(parent) = ours_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let ours_data = storage.get(ours_hash)?;
    let ours_obj = Object::from_bytes(&ours_data).map_err(anyhow::Error::msg)?;
    if let Object::Blob(blob) = ours_obj {
        let mut file = fs::File::create(&ours_path)?;
        for chunk_entry in blob.chunks {
            let chunk_data = storage.get(&chunk_entry.hash)?;
            file.write_all(&chunk_data)?;
        }
    }

    let theirs_path = working_dir.join(format!("{}.theirs", path));
    let theirs_data = storage.get(theirs_hash)?;
    let theirs_obj = Object::from_bytes(&theirs_data).map_err(anyhow::Error::msg)?;
    if let Object::Blob(blob) = theirs_obj {
        let mut file = fs::File::create(&theirs_path)?;
        for chunk_entry in blob.chunks {
            let chunk_data = storage.get(&chunk_entry.hash)?;
            file.write_all(&chunk_data)?;
        }
    }

    Ok(())
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
            ours.cloned()
        } else if base == ours {
            theirs.cloned()
        } else if base == theirs {
            ours.cloned()
        } else {
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
    let (mut merged_tree, conflicts) = merge_trees(
        &base_commit.tree,
        &ours_commit.tree,
        &theirs_commit.tree,
        storage,
    )?;

    merged_tree.sort();
    merged_tree
        .validate()
        .map_err(|e| anyhow::anyhow!("Merged tree validation failed: {}", e))?;

    // Write binary tree
    let mut tree_bytes = Vec::new();
    let payload_hash_bytes = merged_tree
        .write_binary(&mut tree_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to serialize merged tree: {}", e))?;
    let merged_tree_hash = hex::encode(payload_hash_bytes);

    if let Err(e) = storage.put(&merged_tree_hash, &tree_bytes) {
        if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
            return Err(e.into());
        }
    }

    let merge_state = MergeState {
        ours: ours_hash.to_string(),
        theirs: theirs_hash,
        target_branch: target_branch.to_string(),
        conflicts,
        merged_tree_hash,
    };

    Ok(merge_state)
}

fn load_commit(hash: &str, storage: &FilesystemStorage) -> Result<Commit> {
    let data = storage.get(hash)?;
    let obj = Object::from_bytes(&data).map_err(anyhow::Error::msg)?;
    if let Object::Commit(commit) = obj {
        Ok(commit)
    } else {
        anyhow::bail!("Not a commit object")
    }
}
