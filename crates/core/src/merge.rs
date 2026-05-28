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
    pub conflict_details: HashMap<String, (String, String)>, // path -> (ours_blob_hash, theirs_blob_hash)
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

// ─── Conflict tracking (.sdal/CONFLICTS) ───────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConflictEntry {
    pub path: String,
    pub ours_blob: String,
    pub theirs_blob: String,
    pub resolved: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConflictIndex {
    pub entries: Vec<ConflictEntry>,
}

impl ConflictIndex {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn from_merge_state(state: &MergeState) -> Self {
        let entries = state
            .conflicts
            .iter()
            .map(|path| {
                let (ours_blob, theirs_blob) = state
                    .conflict_details
                    .get(path)
                    .cloned()
                    .unwrap_or_default();
                ConflictEntry {
                    path: path.clone(),
                    ours_blob,
                    theirs_blob,
                    resolved: false,
                }
            })
            .collect();
        Self { entries }
    }

    pub fn load(repo_root: &Path) -> Result<Option<Self>> {
        let path = repo_root.join("CONFLICTS");
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let index: ConflictIndex = serde_json::from_str(&content)?;
        Ok(Some(index))
    }

    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let path = repo_root.join("CONFLICTS");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn delete(repo_root: &Path) -> Result<()> {
        let path = repo_root.join("CONFLICTS");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn has_unresolved(&self) -> bool {
        self.entries.iter().any(|e| !e.resolved)
    }

    pub fn unresolved_paths(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| !e.resolved)
            .map(|e| e.path.as_str())
            .collect()
    }

    pub fn mark_resolved(&mut self, path: &str) {
        for entry in &mut self.entries {
            if entry.path == path {
                entry.resolved = true;
            }
        }
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

/// Convert a tree to a flat map of full file paths -> blob hashes, recursing
/// into every subdirectory (e.g. "dir/sub/file.txt").
fn flatten_tree(tree_hash: &str, storage: &FilesystemStorage) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    flatten_tree_into(tree_hash, storage, "", &mut result)?;
    Ok(result)
}

fn flatten_tree_into(
    tree_hash: &str,
    storage: &FilesystemStorage,
    prefix: &str,
    out: &mut HashMap<String, String>,
) -> Result<()> {
    let tree_data = storage.get(tree_hash)?;
    let obj = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

    if let Object::Tree(tree) = obj {
        for (name, entry) in tree.entries {
            let full = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };
            match entry {
                TreeEntry::Blob { hash, .. } => {
                    out.insert(full, hash);
                }
                TreeEntry::Tree { hash } => {
                    flatten_tree_into(&hash, storage, &full, out)?;
                }
            }
        }
    }

    Ok(())
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

/// Build a hierarchical tree from a flat `path -> blob_hash` map, writing every
/// (sub)tree object to storage. Returns the root tree hash. Paths use '/' as the
/// separator (e.g. "dir/sub/file.txt"); each component becomes a nested Tree.
fn build_merged_tree(
    entries: &HashMap<String, String>,
    storage: &FilesystemStorage,
) -> Result<String> {
    let mut files_here: Vec<(String, String)> = Vec::new();
    let mut subdirs: HashMap<String, HashMap<String, String>> = HashMap::new();

    for (path, hash) in entries {
        match path.split_once('/') {
            Some((dir, rest)) => {
                subdirs
                    .entry(dir.to_string())
                    .or_default()
                    .insert(rest.to_string(), hash.clone());
            }
            None => files_here.push((path.clone(), hash.clone())),
        }
    }

    let mut tree = Tree::new();
    for (name, hash) in files_here {
        tree.entries.push((name, TreeEntry::Blob { hash, size: 0 }));
    }
    for (dir, sub) in subdirs {
        let subtree_hash = build_merged_tree(&sub, storage)?;
        tree.entries
            .push((dir, TreeEntry::Tree { hash: subtree_hash }));
    }

    tree.sort();
    tree.validate()
        .map_err(|e| anyhow::anyhow!("Merged tree validation failed: {}", e))?;

    let mut bytes = Vec::new();
    tree.write_binary(&mut bytes)
        .map_err(|e| anyhow::anyhow!("Failed to serialize merged tree: {}", e))?;

    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut hasher, &bytes);
    let hash = hex::encode(sha2::Digest::finalize(hasher));

    if let Err(e) = storage.put(&hash, &bytes) {
        if !matches!(e, sdal_storage::StorageError::AlreadyExists(_)) {
            return Err(e.into());
        }
    }

    Ok(hash)
}

/// Perform 3-way merge on trees (recursively, across subdirectories).
/// Returns (merged_files: path -> blob_hash, conflicts, conflict_details).
pub fn merge_trees(
    base_tree_hash: &str,
    ours_tree_hash: &str,
    theirs_tree_hash: &str,
    storage: &FilesystemStorage,
) -> Result<(HashMap<String, String>, Vec<String>, HashMap<String, (String, String)>)> {
    let base_files = flatten_tree(base_tree_hash, storage)?;
    let ours_files = flatten_tree(ours_tree_hash, storage)?;
    let theirs_files = flatten_tree(theirs_tree_hash, storage)?;

    let mut all_paths = HashSet::new();
    all_paths.extend(base_files.keys().cloned());
    all_paths.extend(ours_files.keys().cloned());
    all_paths.extend(theirs_files.keys().cloned());

    let mut merged_files: HashMap<String, String> = HashMap::new();
    let mut conflicts = Vec::new();
    let mut conflict_details: HashMap<String, (String, String)> = HashMap::new();

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
            // Store blob hashes for both sides so conflict files can be written
            if let (Some(o), Some(t)) = (ours, theirs) {
                conflict_details.insert(path.clone(), (o.clone(), t.clone()));
            }
            None
        };

        if let Some(hash) = result_hash {
            merged_files.insert(path.clone(), hash);
        }
    }

    Ok((merged_files, conflicts, conflict_details))
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

    // 3-way merge across all subdirectories, then rebuild a hierarchical tree
    // (and all subtree objects) from the merged file set.
    let (merged_files, conflicts, conflict_details) = merge_trees(
        &base_commit.tree,
        &ours_commit.tree,
        &theirs_commit.tree,
        storage,
    )?;

    let merged_tree_hash = build_merged_tree(&merged_files, storage)?;

    let merge_state = MergeState {
        ours: ours_hash.to_string(),
        theirs: theirs_hash,
        target_branch: target_branch.to_string(),
        conflicts,
        conflict_details,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_storage() -> FilesystemStorage {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("sdal_merge_{}_{}", std::process::id(), nanos))
            .join(".sdal");
        FilesystemStorage::new(&root).unwrap()
    }

    fn store_tree(tree: &mut Tree, storage: &FilesystemStorage) -> String {
        tree.sort();
        let mut bytes = Vec::new();
        tree.write_binary(&mut bytes).unwrap();
        let mut h = <sha2::Sha256 as sha2::Digest>::new();
        sha2::Digest::update(&mut h, &bytes);
        let hash = hex::encode(sha2::Digest::finalize(h));
        let _ = storage.put(&hash, &bytes);
        hash
    }

    #[test]
    fn flatten_tree_recurses_into_subdirs() {
        let st = tmp_storage();
        let blob = "aa".repeat(32);
        let mut inner = Tree::new();
        inner
            .entries
            .push(("file.txt".into(), TreeEntry::Blob { hash: blob.clone(), size: 0 }));
        let inner_hash = store_tree(&mut inner, &st);
        let mut root = Tree::new();
        root.entries
            .push(("dir".into(), TreeEntry::Tree { hash: inner_hash }));
        let root_hash = store_tree(&mut root, &st);

        let flat = flatten_tree(&root_hash, &st).unwrap();
        assert_eq!(
            flat.get("dir/file.txt"),
            Some(&blob),
            "files inside subdirectories must be flattened with their full path"
        );
    }

    #[test]
    fn build_merged_tree_preserves_subdirs() {
        let st = tmp_storage();
        let mut entries = HashMap::new();
        entries.insert("a.txt".to_string(), "11".repeat(32));
        entries.insert("dir/b.txt".to_string(), "22".repeat(32));
        entries.insert("dir/sub/c.txt".to_string(), "33".repeat(32));

        let root = build_merged_tree(&entries, &st).unwrap();

        // Round-trip: flattening the rebuilt tree must reproduce every path.
        let flat = flatten_tree(&root, &st).unwrap();
        assert_eq!(flat.len(), 3, "all files must survive the hierarchical rebuild");
        assert_eq!(flat.get("a.txt"), Some(&"11".repeat(32)));
        assert_eq!(flat.get("dir/b.txt"), Some(&"22".repeat(32)));
        assert_eq!(flat.get("dir/sub/c.txt"), Some(&"33".repeat(32)));
    }
}
