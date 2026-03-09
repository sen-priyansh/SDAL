use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sdal_chunking::{Chunker, FastCDC};
use sdal_core::{
    Blob, ChunkEntry, Commit, Object, Tree, TreeEntry, checkout, ignore::Ignore, index::Index,
    refs::Refs,
};
use sdal_storage::{FilesystemStorage, Storage};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const BANNER: &str = r#"
  ███████╗██████╗  █████╗ ██╗     
  ██╔════╝██╔══██╗██╔══██╗██║     
  ███████╗██║  ██║███████║██║     
  ╚════██║██║  ██║██╔══██║██║     
  ███████║██████╔╝██║  ██║███████╗
  ╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝

  Sovereign Decentralized Asset Ledger
"#;

#[derive(Parser)]
#[command(name = "sdal")]
#[command(version)]
#[command(about = BANNER, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // Repository Management
    /// Initialize a new SDAL repository
    ///
    /// Creates a new .sdal directory with the necessary structure for version control.
    Init,

    // Staging and Committing
    /// Add files to the staging area
    ///
    /// Stage changes to be included in the next commit. Use '.' to add all files.
    /// Respects patterns in .sdalignore file.
    Add {
        /// Files to add (use '.' for all files in current directory)
        files: Vec<PathBuf>,
    },

    /// Create a new commit from staged changes
    ///
    /// Records a snapshot of all staged changes. If checkpoints exist, uses the
    /// current checkpoint tree and automatically deletes all checkpoints.
    Commit {
        #[arg(short, long)]
        /// Commit message describing the changes
        message: String,
    },

    // History and Status
    /// Show the commit history
    ///
    /// Displays commits in reverse chronological order with hash, author, date, and message.
    /// Use --graph for an ASCII branch graph.
    Log {
        /// Show an ASCII graph of branches and commit history
        #[arg(long)]
        graph: bool,
    },

    /// Show the working directory status
    ///
    /// Displays:
    /// - Staged files (green)
    /// - Modified files (yellow)
    /// - Deleted files (red)
    /// - Untracked files (red)
    Status,

    // Navigation and Recovery
    /// Restore the working directory to a commit (default: last commit)
    ///
    /// With no arguments, discards ALL uncommitted changes and restores
    /// the working directory to the state of the last commit.
    ///
    /// Modes:
    ///   --mode hard:  Move HEAD, unstage, and restore all files (default)
    ///   --mode mixed: Move HEAD and unstage (keep working changes on disk)
    ///   --mode soft:  Move HEAD only (keep staged and working changes)
    Reset {
        /// Commit hash or reference (e.g., HEAD, HEAD~1, HEAD~2)
        #[arg(default_value = "HEAD")]
        commit: String,
        /// Reset mode: hard, mixed, or soft
        #[arg(long, default_value = "hard")]
        mode: String,
    },

    /// Restore files from HEAD
    ///
    /// Discards uncommitted changes and restores files to their state in HEAD.
    Restore {
        /// Files to restore from HEAD
        files: Vec<PathBuf>,
    },

    // Branching
    /// Manage branches
    ///
    /// List, create, or delete branches. Branches are lightweight pointers to commits.
    Branch {
        /// Branch name (optional - if omitted, lists all branches)
        name: Option<String>,
        /// Delete a branch
        #[arg(short = 'd', long)]
        delete: bool,
    },

    /// Switch branches or restore working tree files
    ///
    /// Switch to a different branch, optionally creating it first.
    Checkout {
        /// Branch name to switch to
        branch: String,
        /// Create branch before checking out
        #[arg(short = 'b', long)]
        create: bool,
    },

    /// Merge another branch into current branch
    ///
    /// Performs a 3-way merge. Working directory must be clean.
    /// If conflicts occur, resolve them and run 'sdal commit'.
    Merge {
        /// Branch to merge into current branch
        branch: String,
    },

    // Checkpoints
    /// Manage temporary local snapshots (checkpoints)
    ///
    /// Checkpoints are temporary snapshots for safe experimentation.
    /// They are automatically deleted when you create a commit.
    #[command(subcommand)]
    Checkpoint(CheckpointCommands),

    // Debug
    /// Display a blob object (debug)
    ///
    /// Low-level command to inspect blob contents by hash.
    Cat {
        /// Blob hash to display
        hash: String,
    },

    /// Undo changes by restoring the last saved checkpoint
    ///
    /// Restores your working directory to the most recently saved checkpoint.
    /// If no checkpoints exist, this command does nothing and informs you.
    Undo,

    /// Stage all changes and commit in one step
    ///
    /// Shortcut for 'sdal add . && sdal commit -m <message>'.
    /// Respects .sdalignore patterns. Nothing to stage? Skips the commit.
    Save {
        /// Commit message
        message: String,
    },
}

#[derive(Subcommand)]
enum CheckpointCommands {
    /// Save current working state as a checkpoint
    ///
    /// Creates a temporary snapshot of your current work without creating a commit.
    Save {
        /// Optional description for this checkpoint
        message: Option<String>,
    },

    /// List all saved checkpoints
    ///
    /// Shows all checkpoints with their IDs, messages, and timestamps.
    /// Current checkpoint is marked with *.
    List,

    /// Restore working directory to a checkpoint
    ///
    /// Replaces your working directory with the saved checkpoint state.
    /// Does not affect HEAD or commit history.
    Checkout {
        /// Checkpoint ID (e.g., cp_0001)
        id: String,
    },

    /// Delete a checkpoint
    ///
    /// Removes a checkpoint. This does not affect commits or CAS chunks.
    Drop {
        /// Checkpoint ID to delete
        id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let current_dir = std::env::current_dir()?;
    let sdal_root = current_dir.join(".sdal");

    match cli.command {
        Commands::Init => {
            if sdal_root.exists() {
                println!("SDAL repository already exists.");
                return Ok(());
            }
            fs::create_dir(&sdal_root)?;
            FilesystemStorage::new(&sdal_root)?;

            let refs = Refs::new(&sdal_root);
            refs.create_branch("main", "")?;
            refs.update_head("ref: refs/heads/main")?;

            println!(
                "Initialized empty SDAL repository in {}",
                sdal_root.display()
            );
        }
        Commands::Add { files } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository (run 'sdal init' first)");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;
            let ignore = Ignore::load(&current_dir);
            let mut index = Index::load(&sdal_root)?;

            let mut files_to_add = Vec::new();
            for file_arg in files {
                if file_arg == Path::new(".") {
                    collect_files(&current_dir, &current_dir, &ignore, &mut files_to_add)?;
                } else if file_arg.is_file() {
                    let rel_path = file_arg
                        .strip_prefix(&current_dir)
                        .unwrap_or(&file_arg)
                        .to_string_lossy()
                        .to_string()
                        .replace('\\', "/");
                    if !ignore.should_ignore(&rel_path) {
                        files_to_add.push((file_arg.clone(), rel_path));
                    }
                } else {
                    println!("Skipping {}: not a file", file_arg.display());
                }
            }

            for (path, rel_path) in files_to_add {
                let file_data = std::fs::read(&path)?;
                let total_size = file_data.len() as u64;

                let chunker = FastCDC::new();
                let chunks = chunker.chunk(&file_data)?;

                let mut chunk_entries = Vec::new();
                for chunk in chunks {
                    // Ignore AlreadyExists - chunk deduplication is working
                    match storage.put(&chunk.hash, &chunk.data) {
                        Ok(_) => {}
                        Err(sdal_storage::StorageError::AlreadyExists(_)) => {
                            // Chunk already exists, this is fine (deduplication)
                        }
                        Err(e) => return Err(e.into()),
                    }
                    chunk_entries.push(ChunkEntry::from(&chunk));
                }

                let blob = Blob {
                    chunks: chunk_entries,
                    total_size,
                };
                blob.validate()
                    .map_err(|e| anyhow::anyhow!("Blob validation failed: {}", e))?;

                let blob_object = Object::Blob(blob);
                let blob_json = serde_json::to_vec(&blob_object)?;

                let mut hasher = Sha256::new();
                hasher.update(&blob_json);
                let blob_hash = hex::encode(hasher.finalize());

                // Ignore AlreadyExists - blob deduplication is working
                match storage.put(&blob_hash, &blob_json) {
                    Ok(_) => {}
                    Err(sdal_storage::StorageError::AlreadyExists(_)) => {
                        // Blob already exists, this is fine (deduplication)
                    }
                    Err(e) => return Err(e.into()),
                }

                index.add(rel_path.to_string(), blob_hash);
                println!("add '{}'", rel_path);
            }

            index.save(&sdal_root)?;
        }
        Commands::Cat { hash } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;

            let blob_data = storage
                .get(&hash)
                .with_context(|| format!("Object not found: {}", hash))?;

            // 2. Deserialize
            let object = Object::from_bytes(&blob_data)
                .map_err(anyhow::Error::msg)
                .context("Invalid object format")?;

            // Invariant: validate deserialized object
            object
                .validate()
                .map_err(|e| anyhow::anyhow!("Object validation failed: {}", e))?;

            match object {
                Object::Blob(blob) => {
                    let mut stdout = io::stdout().lock();
                    for chunk_entry in blob.chunks {
                        let chunk_data = storage.get(&chunk_entry.hash)?;
                        stdout.write_all(&chunk_data)?;
                    }
                }
                Object::Tree(_) => {
                    anyhow::bail!("Cannot cat a tree object");
                }
                Object::Commit(_) => {
                    anyhow::bail!("Cannot cat a commit object (use 'sdal log')");
                }
            }
        }
        Commands::Commit { message } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;
            let refs = Refs::new(&sdal_root);
            let mut index = Index::load(&sdal_root)?;

            let tree_hash = if let Some(checkpoint_tree) =
                sdal_checkpoint::ops::get_current_tree(&sdal_root)?
            {
                checkpoint_tree
            } else {
                if index.entries.is_empty() {
                    println!("Nothing to commit (use \"sdal add\" to stage files)");
                    return Ok(());
                }

                build_tree_recursive(&index.entries, &storage)?
            };

            let merge_state = sdal_core::merge::MergeState::load(&sdal_root)?;

            let parents = if let Some(merge_state) = &merge_state {
                // Merge commit: two parents
                vec![merge_state.ours.clone(), merge_state.theirs.clone()]
            } else {
                // Normal commit: one parent (or none for initial)
                refs.read_head()?.into_iter().collect()
            };

            let commit = Commit {
                tree: tree_hash,
                parents,
                author: "user".to_string(), // TODO: Get from config
                message: message.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
            };

            commit
                .validate()
                .map_err(|e| anyhow::anyhow!("Commit validation failed: {}", e))?;

            let commit_object = Object::Commit(commit);
            let commit_json = serde_json::to_vec(&commit_object)?;

            let mut hasher = Sha256::new();
            hasher.update(&commit_json);
            let commit_hash = hex::encode(hasher.finalize());

            storage.put(&commit_hash, &commit_json)?;

            let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
            if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                refs.update_ref(ref_name, &commit_hash)?;
            }

            // Clear index after successful commit
            index.clear();
            index.save(&sdal_root)?;

            // Delete all checkpoints (they are now part of commit history)
            sdal_checkpoint::ops::clear_all_checkpoints(&sdal_root)?;

            if merge_state.is_some() {
                sdal_core::merge::MergeState::delete(&sdal_root)?;
            }

            println!("Created commit {} \"{}\"", &commit_hash[..7], message);
        }
        Commands::Log { graph } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;

            if graph {
                render_log_graph(&sdal_root, &storage)?;
            } else {
                let refs = Refs::new(&sdal_root);
                let mut current = refs.read_head()?;

                if current.is_none() {
                    println!("No commits yet");
                    return Ok(());
                }

                while let Some(commit_hash) = current {
                    if commit_hash.is_empty() {
                        break;
                    }

                    let commit_data = storage.get(&commit_hash)?;
                    let object = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;

                    if let Object::Commit(commit) = object {
                        println!("commit {}", commit_hash);
                        println!("Author: {}", commit.author);
                        println!("Date:   {}", commit.timestamp);
                        println!("\n    {}\n", commit.message);
                        current = commit.parents.first().cloned();
                    } else {
                        break;
                    }
                }
            }
        }
        Commands::Status => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let ignore = Ignore::load(&current_dir);
            let index = Index::load(&sdal_root)?;
            let refs = Refs::new(&sdal_root);
            let storage = FilesystemStorage::new(&sdal_root)?;

            println!("\n-- SDAL VCS STATUS --\n");

            // === SECTION 1: WORKING DIRECTORY ===
            println!("[ WORKING DIRECTORY ]");

            let mut has_changes = false;

            // Get HEAD files for comparison
            let mut head_files = std::collections::HashMap::new();
            if let Some(head_hash) = refs.read_head()? {
                if let Ok(commit_data) = storage.get(&head_hash) {
                    if let Ok(Object::Commit(commit)) = Object::from_bytes(&commit_data) {
                        collect_tree_files(&commit.tree, "", &storage, &mut head_files);
                    }
                }
            }

            let mut all_files = Vec::new();
            collect_files(&current_dir, &current_dir, &ignore, &mut all_files)?;

            let mut staged = Vec::new();
            let mut modified = Vec::new();
            let mut untracked = Vec::new();
            let mut deleted = Vec::new();

            for (path, rel_path) in &all_files {
                if index.is_staged(&rel_path) {
                    // Check if staged file has been modified since staging
                    if let Some(index_hash) = index.entries.get(rel_path) {
                        let file_data = std::fs::read(path)?;
                        let chunker = FastCDC::new();
                        let chunks = chunker.chunk(&file_data)?;

                        let mut chunk_entries = Vec::new();
                        for chunk in chunks {
                            chunk_entries.push(ChunkEntry::from(&chunk));
                        }

                        let blob = Blob {
                            chunks: chunk_entries,
                            total_size: file_data.len() as u64,
                        };

                        let blob_object = Object::Blob(blob);
                        let blob_json = serde_json::to_vec(&blob_object)?;

                        let mut hasher = Sha256::new();
                        hasher.update(&blob_json);
                        let current_hash = hex::encode(hasher.finalize());

                        if &current_hash == index_hash {
                            staged.push(rel_path.clone());
                        } else {
                            // Staged but modified since staging
                            modified.push(rel_path.clone());
                        }
                    } else {
                        staged.push(rel_path.clone());
                    }
                } else if let Some(head_hash) = head_files.get(rel_path) {
                    // File exists in HEAD - check if actually modified
                    let file_data = std::fs::read(path)?;
                    let chunker = FastCDC::new();
                    let chunks = chunker.chunk(&file_data)?;

                    let mut chunk_entries = Vec::new();
                    for chunk in chunks {
                        chunk_entries.push(ChunkEntry::from(&chunk));
                    }

                    let blob = Blob {
                        chunks: chunk_entries,
                        total_size: file_data.len() as u64,
                    };

                    let blob_object = Object::Blob(blob);
                    let blob_json = serde_json::to_vec(&blob_object)?;

                    let mut hasher = Sha256::new();
                    hasher.update(&blob_json);
                    let current_hash = hex::encode(hasher.finalize());

                    if &current_hash != head_hash {
                        modified.push(rel_path.clone());
                    }
                    // If hashes match, file is unchanged - don't show it
                } else {
                    untracked.push(rel_path.clone());
                }
            }

            let working_files: std::collections::HashSet<_> = all_files
                .iter()
                .map(|(_, rel_path)| rel_path.as_str())
                .collect();
            for (path, _) in &head_files {
                if !working_files.contains(path.as_str()) && !index.is_staged(path) {
                    deleted.push(path.clone());
                }
            }

            for file in &staged {
                println!("  \x1b[32mA\x1b[0m  {}         (staged)", file);
                has_changes = true;
            }

            for file in &modified {
                println!("  \x1b[33mM\x1b[0m  {}         (modified)", file);
                has_changes = true;
            }

            for file in &deleted {
                println!("  \x1b[31mD\x1b[0m  {}         (deleted)", file);
                has_changes = true;
            }

            for file in &untracked {
                println!("  \x1b[31m?\x1b[0m  {}         (untracked)", file);
                has_changes = true;
            }

            if !has_changes {
                println!("  (clean - no changes)");
            }

            println!();

            // === SECTION 2: GHOST CHECKPOINTS ===
            println!("[ GHOST CHECKPOINTS ]");

            let checkpoint_index = sdal_checkpoint::ops::list_checkpoints(&sdal_root)?;
            if checkpoint_index.checkpoints.is_empty() {
                println!("  (none)");
            } else {
                for cp in &checkpoint_index.checkpoints {
                    let is_current = Some(&cp.id) == checkpoint_index.current.as_ref();
                    let marker = if is_current { " *" } else { "" };

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs() as i64;
                    let diff = now - cp.timestamp;
                    let time_str = if diff < 60 {
                        format!("{} secs ago", diff)
                    } else if diff < 3600 {
                        format!("{} mins ago", diff / 60)
                    } else if diff < 86400 {
                        format!("{} hours ago", diff / 3600)
                    } else {
                        format!("{} days ago", diff / 86400)
                    };

                    let msg = cp.message.as_deref().unwrap_or("(no message)");
                    println!("  ○  {}  \"{}\" ({}){}", &cp.id, msg, time_str, marker);
                }
            }

            println!();

            // === SECTION 3: LEDGER HEAD ===
            println!("[ LEDGER HEAD ]");

            if let Some(head_hash) = refs.read_head()? {
                if let Ok(commit_data) = storage.get(&head_hash) {
                    if let Ok(Object::Commit(commit)) = Object::from_bytes(&commit_data) {
                        let time_str = {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)?
                                .as_secs() as i64;
                            let diff = now - commit.timestamp;
                            if diff < 3600 {
                                format!("{} mins ago", diff / 60)
                            } else if diff < 86400 {
                                format!("{} hours ago", diff / 3600)
                            } else {
                                format!("{} days ago", diff / 86400)
                            }
                        };

                        println!(
                            "  ●  {}  \"{}\" ({}, {})",
                            &head_hash[..7],
                            commit.message,
                            commit.author,
                            time_str
                        );
                    }
                }
            } else {
                println!("  (no commits yet)");
            }

            println!("\n-----------------------");
            if !checkpoint_index.checkpoints.is_empty() {
                println!("Hint: Use 'sdal commit' to solidify checkpoints into ledger");
            } else if has_changes {
                println!("Hint: Use 'sdal add' to stage changes, then 'sdal commit'");
            }
            println!();
        }
        Commands::Reset { commit, mode } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;
            let refs = Refs::new(&sdal_root);
            let mut index = Index::load(&sdal_root)?;

            // Resolve commit reference with partial matching
            let target_hash = if commit == "HEAD" {
                refs.read_head()?.ok_or(anyhow::anyhow!("No commits yet"))?
            } else if commit.starts_with("HEAD~") {
                // Simple HEAD~N support
                let steps = commit
                    .strip_prefix("HEAD~")
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or(anyhow::anyhow!("Invalid reference: {}", commit))?;

                let mut current = refs.read_head()?.ok_or(anyhow::anyhow!("No commits yet"))?;
                for _ in 0..steps {
                    let commit_data = storage.get(&current)?;
                    let obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;
                    if let Object::Commit(c) = obj {
                        current = c
                            .parents
                            .first()
                            .ok_or(anyhow::anyhow!("No more commits"))?
                            .clone();
                    }
                }
                current
            } else {
                // Assume it's a hash or partial hash
                // Try to resolve partial hash
                if commit.len() < 64 {
                    if commit.len() < 64 {
                        anyhow::bail!(
                            "Short hashes not yet supported. Please use the full 64-character hash from 'sdal log'."
                        );
                    }
                }
                commit.clone()
            };

            match mode.as_str() {
                "soft" => {
                    let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
                    if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                        refs.update_ref(ref_name, &target_hash)?;
                    }

                    // Note: Reset soft usually leaves index/workdir alone
                    println!("HEAD moved to {}", &target_hash[..7]);
                }
                "mixed" => {
                    let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
                    if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                        refs.update_ref(ref_name, &target_hash)?;
                    }
                    index.clear();
                    index.save(&sdal_root)?;
                    println!("Unstaged changes after reset:");
                    println!("HEAD moved to {}", &target_hash[..7]);
                }
                "hard" => {
                    let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
                    if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                        refs.update_ref(ref_name, &target_hash)?;
                    }
                    index.clear();
                    index.save(&sdal_root)?;

                    // Restore working directory from commit (clean version removes extra files)
                    let commit_data = storage.get(&target_hash)?;
                    let obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;
                    if let Object::Commit(commit) = obj {
                        checkout::restore_tree_clean(&commit.tree, &storage, &current_dir, &Ignore::load(&current_dir))?;
                    }

                    println!(
                        "HEAD is now at {} (working directory restored)",
                        &target_hash[..7]
                    );
                }
                _ => anyhow::bail!("Invalid mode: {}. Use soft, mixed, or hard", mode),
            }
        }
        Commands::Restore { files } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;
            let refs = Refs::new(&sdal_root);

            let head_hash = refs.read_head()?.ok_or(anyhow::anyhow!("No commits yet"))?;
            let commit_data = storage.get(&head_hash)?;
            let obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;

            if let Object::Commit(commit) = obj {
                let tree_data = storage.get(&commit.tree)?;
                let tree_obj = Object::from_bytes(&tree_data).map_err(anyhow::Error::msg)?;

                if let Object::Tree(tree) = tree_obj {
                    for file_path in files {
                        let rel_path = file_path
                            .strip_prefix(&current_dir)
                            .unwrap_or(&file_path)
                            .to_string_lossy()
                            .to_string();

                        for (name, entry) in &tree.entries {
                            if name == &rel_path {
                                if let TreeEntry::Blob { hash, .. } = entry {
                                    checkout::restore_blob(hash, &storage, &file_path)?;
                                    println!("Restored '{}'", rel_path);
                                }
                            }
                        }
                    }
                }
            }
        }
        Commands::Branch { name, delete } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let refs = Refs::new(&sdal_root);

            if delete {
                let branch_name = name
                    .as_ref()
                    .ok_or(anyhow::anyhow!("Branch name required for deletion"))?;
                refs.delete_branch(branch_name)?;
                println!("Deleted branch '{}'", branch_name);
            } else if let Some(branch_name) = name {
                let head_hash = refs.read_head()?.ok_or(anyhow::anyhow!(
                    "No commits yet - create an initial commit first"
                ))?;
                refs.create_branch(&branch_name, &head_hash)?;
                println!("Created branch '{}'", branch_name);
            } else {
                let branches = refs.list_branches()?;
                let current = refs.get_current_branch()?;

                if branches.is_empty() {
                    println!("No branches yet");
                } else {
                    for branch in branches {
                        let marker = if Some(&branch) == current.as_ref() {
                            "* "
                        } else {
                            "  "
                        };
                        println!("{}{}", marker, branch);
                    }
                }
            }
        }
        Commands::Checkout { branch, create } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let refs = Refs::new(&sdal_root);
            let storage = FilesystemStorage::new(&sdal_root)?;

            if create {
                let head_hash = refs.read_head()?.ok_or(anyhow::anyhow!("No commits yet"))?;
                refs.create_branch(&branch, &head_hash)?;
                println!("Created branch '{}'", branch);
            }

            let branch_ref = format!("refs/heads/{}", branch);
            let target_hash = refs
                .read_ref(&branch_ref)?
                .ok_or(anyhow::anyhow!("Branch '{}' not found", branch))?;

            let commit_data = storage.get(&target_hash)?;
            let commit_obj = Object::from_bytes(&commit_data).map_err(anyhow::Error::msg)?;

            if let Object::Commit(commit) = commit_obj {
                // Restore working directory from commit tree (clean version removes extra files)
                checkout::restore_tree_clean(&commit.tree, &storage, &current_dir, &Ignore::load(&current_dir))?;

                refs.switch_branch(&branch)?;

                println!("Switched to branch '{}'", branch);
            } else {
                anyhow::bail!("Invalid commit object");
            }
        }
        Commands::Merge { branch } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            let refs = Refs::new(&sdal_root);
            let storage = FilesystemStorage::new(&sdal_root)?;
            let index = Index::load(&sdal_root)?;

            // Safety check 1: Must be on a branch
            let current_branch = refs
                .get_current_branch()?
                .ok_or(anyhow::anyhow!("Cannot merge in detached HEAD state"))?;

            // Safety check 2: Cannot merge branch into itself
            if current_branch == *branch {
                anyhow::bail!("Cannot merge branch '{}' into itself", branch);
            }

            // Safety check 3: Working directory must be clean (no uncommitted changes)
            if !index.entries.is_empty() {
                anyhow::bail!("Cannot merge with uncommitted changes. Commit or stash them first.");
            }

            // Safety check 4: No checkpoints allowed during merge
            let checkpoint_index = sdal_checkpoint::ops::list_checkpoints(&sdal_root)?;
            if !checkpoint_index.checkpoints.is_empty() {
                anyhow::bail!("Cannot merge with active checkpoints. Commit or drop them first.");
            }

            let ours_hash = refs.read_head()?.ok_or(anyhow::anyhow!("No commits yet"))?;

            // Perform merge
            let merge_state =
                sdal_core::merge::perform_merge(&branch, &ours_hash, &sdal_root, &storage)?;

            checkout::restore_tree_clean(&merge_state.merged_tree_hash, &storage, &current_dir, &Ignore::load(&current_dir))?;

            // Update index to match merged tree
            let mut index = Index::load(&sdal_root)?;
            index.clear();
            sdal_core::merge::populate_index_from_tree(
                &merge_state.merged_tree_hash,
                &storage,
                &mut index,
                "",
            )?;
            index.save(&sdal_root)?;

            if merge_state.conflicts.is_empty() {
                println!("Merge successful! No conflicts.");
                println!("Run 'sdal commit' to finalize the merge.");

                merge_state.save(&sdal_root)?;
            } else {
                println!("Merge conflict! Conflicts in:");
                for conflict in &merge_state.conflicts {
                    println!("  - {}", conflict);

                    let ours_commit_data = storage.get(&merge_state.ours)?;
                    let ours_obj =
                        Object::from_bytes(&ours_commit_data).map_err(anyhow::Error::msg)?;
                    let theirs_commit_data = storage.get(&merge_state.theirs)?;
                    let theirs_obj =
                        Object::from_bytes(&theirs_commit_data).map_err(anyhow::Error::msg)?;

                    if let (Object::Commit(_ours_commit), Object::Commit(_theirs_commit)) =
                        (ours_obj, theirs_obj)
                    {
                        // TODO: Implement proper tree walking to get blob hashes
                    }
                }
                println!("\nConflict files written as .ours and .theirs");
                println!("Resolve conflicts manually, then run 'sdal commit'");

                merge_state.save(&sdal_root)?;
            }
        }
        Commands::Checkpoint(cmd) => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }

            match cmd {
                CheckpointCommands::Save { message } => {
                    let id = sdal_checkpoint::ops::save_checkpoint(
                        &sdal_root,
                        &current_dir,
                        message.clone(),
                    )?;
                    println!("Saved checkpoint: {}", id);
                    if let Some(msg) = message {
                        println!("  Message: {}", msg);
                    }
                }
                CheckpointCommands::List => {
                    let index = sdal_checkpoint::ops::list_checkpoints(&sdal_root)?;

                    if index.checkpoints.is_empty() {
                        println!("No checkpoints");
                    } else {
                        println!("Checkpoints:");
                        for cp in &index.checkpoints {
                            let current_marker = if Some(&cp.id) == index.current.as_ref() {
                                " *"
                            } else {
                                ""
                            };
                            println!("  {}{}", cp.id, current_marker);
                            if let Some(msg) = &cp.message {
                                println!("    Message: {}", msg);
                            }
                            println!("    Tree: {}", &cp.tree_root[..7]);
                            println!("    Time: {}", cp.timestamp);
                        }
                    }
                }
                CheckpointCommands::Checkout { id } => {
                    sdal_checkpoint::ops::checkout_checkpoint(&sdal_root, &current_dir, &id)?;
                    println!("Checked out to checkpoint: {}", id);
                }
                CheckpointCommands::Drop { id } => {
                    sdal_checkpoint::ops::drop_checkpoint(&sdal_root, &id)?;
                    println!("Dropped checkpoint: {}", id);
                }
            }
        }
        Commands::Save { message } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository (run 'sdal init' first)");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;
            let ignore = Ignore::load(&current_dir);
            let mut index = Index::load(&sdal_root)?;

            // --- Stage all files (same logic as `sdal add .`) ---
            let mut files_to_add = Vec::new();
            collect_files(&current_dir, &current_dir, &ignore, &mut files_to_add)?;

            if files_to_add.is_empty() {
                println!("Nothing to save — working directory is empty or all files are ignored.");
                return Ok(());
            }

            let mut staged_count = 0usize;
            for (path, rel_path) in files_to_add {
                let file_data = std::fs::read(&path)?;
                let total_size = file_data.len() as u64;

                let chunker = sdal_chunking::FastCDC::new();
                let chunks = chunker.chunk(&file_data)?;

                let mut chunk_entries = Vec::new();
                for chunk in chunks {
                    match storage.put(&chunk.hash, &chunk.data) {
                        Ok(_) => {}
                        Err(sdal_storage::StorageError::AlreadyExists(_)) => {}
                        Err(e) => return Err(e.into()),
                    }
                    chunk_entries.push(ChunkEntry::from(&chunk));
                }

                let blob = Blob {
                    chunks: chunk_entries,
                    total_size,
                };
                blob.validate()
                    .map_err(|e| anyhow::anyhow!("Blob validation failed: {}", e))?;

                let blob_object = Object::Blob(blob);
                let blob_json = serde_json::to_vec(&blob_object)?;

                let mut hasher = Sha256::new();
                hasher.update(&blob_json);
                let blob_hash = hex::encode(hasher.finalize());

                match storage.put(&blob_hash, &blob_json) {
                    Ok(_) => {}
                    Err(sdal_storage::StorageError::AlreadyExists(_)) => {}
                    Err(e) => return Err(e.into()),
                }

                index.add(rel_path.to_string(), blob_hash);
                staged_count += 1;
            }

            index.save(&sdal_root)?;
            println!("Staged {} file(s).", staged_count);

            // --- Commit (same logic as `sdal commit`) ---
            let refs = Refs::new(&sdal_root);

            let tree_hash = if let Some(checkpoint_tree) =
                sdal_checkpoint::ops::get_current_tree(&sdal_root)?
            {
                checkpoint_tree
            } else {
                build_tree_recursive(&index.entries, &storage)?
            };

            let merge_state = sdal_core::merge::MergeState::load(&sdal_root)?;

            let parents = if let Some(merge_state) = &merge_state {
                vec![merge_state.ours.clone(), merge_state.theirs.clone()]
            } else {
                refs.read_head()?.into_iter().collect()
            };

            let commit = Commit {
                tree: tree_hash,
                parents,
                author: "user".to_string(),
                message: message.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
            };

            commit
                .validate()
                .map_err(|e| anyhow::anyhow!("Commit validation failed: {}", e))?;

            let commit_object = Object::Commit(commit);
            let commit_json = serde_json::to_vec(&commit_object)?;

            let mut hasher = Sha256::new();
            hasher.update(&commit_json);
            let commit_hash = hex::encode(hasher.finalize());

            storage.put(&commit_hash, &commit_json)?;

            let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
            if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                refs.update_ref(ref_name, &commit_hash)?;
            }

            index.clear();
            index.save(&sdal_root)?;

            sdal_checkpoint::ops::clear_all_checkpoints(&sdal_root)?;

            if merge_state.is_some() {
                sdal_core::merge::MergeState::delete(&sdal_root)?;
            }

            println!("Saved! Created commit {} \"{}\"", &commit_hash[..7], message);
        }
        Commands::Undo => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository (run 'sdal init' first)");
            }

            let index = sdal_checkpoint::ops::list_checkpoints(&sdal_root)?;

            if index.checkpoints.is_empty() {
                println!("Nothing to undo — no checkpoints saved.");
                println!("Tip: use 'sdal checkpoint save <message>' to save a snapshot first.");
                return Ok(());
            }

            // The last checkpoint in the list is the most recently saved one
            let last = index.checkpoints.last().unwrap();
            let id = last.id.clone();
            let msg = last
                .message
                .clone()
                .unwrap_or_else(|| "(no message)".to_string());

            sdal_checkpoint::ops::checkout_checkpoint(&sdal_root, &current_dir, &id)?;

            println!("Undone to checkpoint {} — \"{}\"", id, msg);
            println!("Tip: your other checkpoints are still saved. Use 'sdal checkpoint list' to see them.");
        }
    }

    Ok(())
}

/// Recursively collect files from a directory
fn collect_files(
    root: &Path,
    dir: &Path,
    ignore: &Ignore,
    results: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        if ignore.should_ignore(&rel_path) {
            continue;
        }

        if path.is_file() {
            results.push((path, rel_path));
        } else if path.is_dir() {
            collect_files(root, &path, ignore, results)?;
        }
    }
    Ok(())
}

/// Recursively collect all blob paths and their hashes from a tree,
/// building full relative paths (e.g. "src/main.rs") separated by '/'.
fn collect_tree_files(
    tree_hash: &str,
    prefix: &str,
    storage: &FilesystemStorage,
    out: &mut std::collections::HashMap<String, String>,
) {
    let data = match storage.get(tree_hash) {
        Ok(d) => d,
        Err(_) => return,
    };
    let tree = match Object::from_bytes(&data) {
        Ok(Object::Tree(t)) => t,
        _ => return,
    };
    for (name, entry) in tree.entries {
        let full_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        match entry {
            TreeEntry::Blob { hash, .. } => {
                out.insert(full_path, hash);
            }
            TreeEntry::Tree { hash } => {
                collect_tree_files(&hash, &full_path, storage, out);
            }
        }
    }
}


fn build_tree_recursive(
    entries: &std::collections::HashMap<String, String>,
    storage: &FilesystemStorage,
) -> Result<String> {
    let mut tree = Tree::new();
    let mut subfolders: std::collections::HashMap<
        String,
        std::collections::HashMap<String, String>,
    > = std::collections::HashMap::new();
    let mut files_at_this_level = std::collections::HashSet::new();

    for (path, hash) in entries {
        if let Some(pos) = path.find('/') {
            let (dir_name, remaining) = path.split_at(pos);
            let remaining = &remaining[1..]; // skip '/'

            if files_at_this_level.contains(dir_name) {
                anyhow::bail!(
                    "File/directory name collision: '{}' exists both as a file and as a directory path",
                    dir_name
                );
            }

            subfolders
                .entry(dir_name.to_string())
                .or_default()
                .insert(remaining.to_string(), hash.clone());
        } else {
            if subfolders.contains_key(path.as_str()) {
                anyhow::bail!(
                    "File/directory name collision: '{}' exists both as a file and as a directory path",
                    path
                );
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

    let mut tree_bytes = Vec::new();
    tree.write_binary(&mut tree_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to serialize tree: {}", e))?;

    // Hash the ENTIRE serialized tree (including headers)
    // This must match what storage.put() will verify
    let mut hasher = Sha256::new();
    hasher.update(&tree_bytes);
    let tree_hash = hex::encode(hasher.finalize());

    match storage.put(&tree_hash, &tree_bytes) {
        Ok(_) => {}
        Err(sdal_storage::StorageError::AlreadyExists(_)) => {} // deduplication — this is fine
        Err(e) => return Err(e.into()),
    }
    Ok(tree_hash)
}

/// Format a duration in seconds as a human-readable "X ago" string.
fn format_time_diff(diff_secs: i64) -> String {
    if diff_secs < 0 {
        return "just now".to_string();
    }
    if diff_secs < 60 {
        return format!("{} secs ago", diff_secs);
    }
    if diff_secs < 3600 {
        return format!("{} mins ago", diff_secs / 60);
    }
    if diff_secs < 86400 {
        return format!("{} hours ago", diff_secs / 3600);
    }
    format!("{} days ago", diff_secs / 86400)
}

/// Render an ASCII commit graph to stdout.
///
/// Uses a lane-based algorithm: each "lane" (column) tracks the hash of the
/// next commit expected in that column. As commits are printed top-to-bottom
/// (newest first), lanes are updated, forked (merge commits), and joined
/// (shared ancestors).
fn render_log_graph(sdal_root: &Path, storage: &FilesystemStorage) -> Result<()> {
    use std::collections::{HashMap, HashSet, VecDeque};

    let refs = Refs::new(sdal_root);

    // Build map: commit hash → list of branch names that point to it
    let mut branch_tips: HashMap<String, Vec<String>> = HashMap::new();
    let current_branch = refs.get_current_branch()?;
    for branch in refs.list_branches()? {
        let ref_path = format!("refs/heads/{}", branch);
        if let Some(hash) = refs.read_ref(&ref_path)? {
            branch_tips.entry(hash).or_default().push(branch.clone());
        }
    }

    let head_hash = match refs.read_head()? {
        Some(h) if !h.is_empty() => h,
        _ => {
            println!("No commits yet");
            return Ok(());
        }
    };

    // BFS from HEAD to collect all reachable commits
    let mut commit_map: HashMap<String, sdal_core::Commit> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(head_hash.clone());

    while let Some(hash) = queue.pop_front() {
        if hash.is_empty() || visited.contains(&hash) {
            continue;
        }
        visited.insert(hash.clone());
        if let Ok(data) = storage.get(&hash) {
            if let Ok(Object::Commit(commit)) = Object::from_bytes(&data) {
                for p in &commit.parents {
                    if !p.is_empty() {
                        queue.push_back(p.clone());
                    }
                }
                commit_map.insert(hash, commit);
            }
        }
    }

    // Sort by timestamp descending (newest first)
    let mut ordered: Vec<(String, sdal_core::Commit)> = commit_map.into_iter().collect();
    ordered.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    // lanes[i] = Some(hash) means lane i is waiting for commit `hash`
    let mut lanes: Vec<Option<String>> = vec![Some(head_hash.clone())];
    let n_commits = ordered.len();

    for (commit_idx, (hash, commit)) in ordered.iter().enumerate() {
        // Find all lanes that point to this commit
        let matching: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                if l.as_deref() == Some(hash.as_str()) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        if matching.is_empty() {
            continue; // unreachable from starting lanes
        }

        let primary = matching[0];
        let is_head = hash == &head_hash;
        let symbol = if is_head { "●" } else { "○" };

        // Parents (filter empty strings)
        let parents: Vec<String> = commit
            .parents
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect();
        let is_merge = parents.len() >= 2;

        // Build the prefix (lanes before primary)
        let mut prefix = String::new();
        for i in 0..primary {
            match &lanes[i] {
                Some(_) => prefix.push_str("│ "),
                None => prefix.push_str("  "),
            }
        }

        // Build the suffix (lanes after primary, excluding secondary matching lanes)
        let mut suffix = String::new();
        for i in (primary + 1)..lanes.len() {
            if matching[1..].contains(&i) {
                suffix.push_str("  "); // this lane is converging here
            } else {
                match &lanes[i] {
                    Some(_) => suffix.push_str("│ "),
                    None => suffix.push_str("  "),
                }
            }
        }
        if is_merge {
            suffix.push_str("  "); // placeholder for the new lane that appears below
        }
        let _suffix = suffix.trim_end();

        // Build ref/branch label
        let time_str = format_time_diff(now_secs - commit.timestamp);
        let short_hash = &hash[..7];
        let ref_label = branch_tips.get(hash).map(|names| {
            let formatted = names
                .iter()
                .map(|b| {
                    if Some(b) == current_branch.as_ref() {
                        format!("HEAD -> {}", b)
                    } else {
                        b.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(" ({})", formatted)
        });

        // Print commit line
        println!(
            "  {}{}  {}  \"{}\" ({}{})",
            prefix,
            symbol,
            short_hash,
            commit.message,
            time_str,
            ref_label.as_deref().unwrap_or("")
        );

        // ── Update lanes ──────────────────────────────────────────────────
        // Secondary matching lanes (convergence) are now closed
        for &sec in &matching[1..] {
            lanes[sec] = None;
        }
        // Primary lane advances to first parent (or closes if none)
        lanes[primary] = parents.first().cloned();
        // Merge: insert second parent into a new lane after primary
        if is_merge {
            let p1 = &parents[1];
            let already_in_lane = lanes.iter().any(|l| l.as_deref() == Some(p1));
            if !already_in_lane {
                lanes.insert(primary + 1, Some(p1.clone()));
            }
        }
        // Trim trailing empty lanes
        while lanes.last() == Some(&None) {
            lanes.pop();
        }

        // ── Connector / separator lines ───────────────────────────────────
        if commit_idx >= n_commits - 1 {
            continue; // last commit — no more lines needed
        }

        // Detect convergence: two or more lanes now point to the same hash
        let mut seen_hashes: HashSet<&str> = HashSet::new();
        let has_convergence = lanes.iter().flatten().any(|h| !seen_hashes.insert(h));

        if is_merge {
            // Fork connector: ├─╮
            let mut fork_line = String::new();
            for i in 0..primary {
                match &lanes[i] {
                    Some(_) => fork_line.push_str("│ "),
                    None => fork_line.push_str("  "),
                }
            }
            fork_line.push_str("├─╮");
            println!("  {}", fork_line);
        }

        if has_convergence {
            // Join connector: ├─╯  (collapse two lanes into one)
            // Find the two lanes that share a hash
            let mut first_lane = 0usize;
            let mut found = false;
            'outer: for i in 0..lanes.len() {
                if let Some(h) = &lanes[i] {
                    for j in (i + 1)..lanes.len() {
                        if lanes[j].as_deref() == Some(h.as_str()) {
                            first_lane = i;
                            found = true;
                            break 'outer;
                        }
                    }
                }
            }
            if found {
                let mut join_line = String::new();
                for i in 0..first_lane {
                    match &lanes[i] {
                        Some(_) => join_line.push_str("│ "),
                        None => join_line.push_str("  "),
                    }
                }
                join_line.push_str("├─╯");
                println!("  {}", join_line);
            }
        } else {
            // Regular separator line
            let mut sep = String::new();
            for lane in &lanes {
                match lane {
                    Some(_) => sep.push_str("│ "),
                    None => sep.push_str("  "),
                }
            }
            let sep = sep.trim_end();
            if !sep.is_empty() {
                println!("  {}", sep);
            }
        }
    }

    Ok(())
}
