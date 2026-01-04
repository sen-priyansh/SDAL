use clap::{Parser, Subcommand};
use anyhow::{Result, Context};
use sdal_chunking::{FixedSizeChunker, Chunker};
use sdal_storage::{FilesystemStorage, Storage};
use sdal_core::{Blob, Object, ChunkEntry, Commit, Tree, TreeEntry, refs::Refs, workdir, index::Index, ignore::Ignore};
use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use std::io::{self, Write};

#[derive(Parser)]
#[command(name = "sdal")]
#[command(about = "Sovereign Decentralized Asset Ledger", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new SDAL repository
    Init,
    /// Add files to staging area
    Add {
        /// Files to add (use '.' for all)
        files: Vec<PathBuf>,
    },
    /// Cat a blob object
    Cat {
        hash: String,
    },
    /// Create a new commit
    Commit {
        #[arg(short, long)]
        message: String,
    },
    /// Show commit history
    Log,
    /// Show working directory status
    Status,
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
            
            // Create refs structure
            let refs = Refs::new(&sdal_root);
            refs.create_branch("main", "")?; // Empty initial branch
            refs.update_head("ref: refs/heads/main")?;
            
            println!("Initialized empty SDAL repository in {}", sdal_root.display());
        }
        Commands::Add { files } => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository (run 'sdal init' first)");
            }
            
            let storage = FilesystemStorage::new(&sdal_root)?;
            let ignore = Ignore::load(&current_dir);
            let mut index = Index::load(&sdal_root)?;
            
            // Collect files to add
            let mut files_to_add = Vec::new();
            for file_arg in files {
                if file_arg == Path::new(".") {
                    // Add all files recursively
                    collect_files(&current_dir, &current_dir, &ignore, &mut files_to_add)?;
                } else if file_arg.is_file() {
                    let rel_path = file_arg.strip_prefix(&current_dir)
                        .unwrap_or(&file_arg)
                        .to_string_lossy()
                        .to_string();
                    if !ignore.should_ignore(&rel_path) {
                        files_to_add.push((file_arg.clone(), rel_path));
                    }
                } else {
                    println!("Skipping {}: not a file", file_arg.display());
                }
            }
            
            // Stage each file
            for (path, rel_path) in files_to_add {
                let data = fs::read(&path)?;
                
                // Create blob
                let chunker = FixedSizeChunker::new(1024 * 1024);
                let chunks = chunker.chunk(&data)?;
                
                let mut chunk_entries = Vec::new();
                for chunk in chunks {
                    storage.put(&chunk.hash, &chunk.data)?;
                    chunk_entries.push(ChunkEntry::from(&chunk));
                }
                
                let blob = Blob {
                    chunks: chunk_entries,
                    total_size: data.len() as u64,
                };
                blob.validate().map_err(|e| anyhow::anyhow!("Blob validation failed: {}", e))?;
                
                let object = Object::Blob(blob);
                let blob_json = serde_json::to_vec(&object)?;
                
                let mut hasher = Sha256::new();
                hasher.update(&blob_json);
                let blob_hash = hex::encode(hasher.finalize());
                
                storage.put(&blob_hash, &blob_json)?;
                index.add(rel_path.clone(), blob_hash);
                
                println!("add '{}'", rel_path);
            }
            
            index.save(&sdal_root)?;
        }
        Commands::Cat { hash } => {
            if !sdal_root.exists() {
               anyhow::bail!("Not an SDAL repository");
            }

            let storage = FilesystemStorage::new(&sdal_root)?;

            // 1. Get Blob
            let blob_data = storage.get(&hash).with_context(|| format!("Object not found: {}", hash))?;
            
            // 2. Deserialize
            let object: Object = serde_json::from_slice(&blob_data).context("Invalid object format")?;
            
            // Invariant: validate deserialized object
            object.validate().map_err(|e| anyhow::anyhow!("Object validation failed: {}", e))?;
            
            match object {
                Object::Blob(blob) => {
                     // 3. Reconstruct
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
            
            if index.entries.is_empty() {
                println!("Nothing to commit (use \"sdal add\" to stage files)");
                return Ok(());
            }
            
            // Build tree from index
            let mut tree = Tree::new();
            for (path, blob_hash) in &index.entries {
                tree.add_entry(
                    path.clone(),
                    TreeEntry::Blob {
                        hash: blob_hash.clone(),
                        size: 0, // TODO: Track size in index
                    },
                );
            }
            
            tree.validate().map_err(|e| anyhow::anyhow!("Tree validation failed: {}", e))?;
            let tree_object = Object::Tree(tree);
            let tree_json = serde_json::to_vec(&tree_object)?;
            
            let mut hasher = Sha256::new();
            hasher.update(&tree_json);
            let tree_hash = hex::encode(hasher.finalize());
            
            storage.put(&tree_hash, &tree_json)?;
            
            // Get parent commit (if any)
            let parent = refs.read_head()?;
            
            // Create commit
            let commit = Commit {
                tree: tree_hash,
                parent,
                author: "user".to_string(), // TODO: Get from config
                message: message.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
            };
            
            commit.validate().map_err(|e| anyhow::anyhow!("Commit validation failed: {}", e))?;
            
            let commit_object = Object::Commit(commit);
            let commit_json = serde_json::to_vec(&commit_object)?;
            
            let mut hasher = Sha256::new();
            hasher.update(&commit_json);
            let commit_hash = hex::encode(hasher.finalize());
            
            storage.put(&commit_hash, &commit_json)?;
            
            // Update current branch
            let head_content = fs::read_to_string(sdal_root.join("HEAD"))?;
            if let Some(ref_name) = head_content.trim().strip_prefix("ref: ") {
                refs.update_ref(ref_name, &commit_hash)?;
            }
            
            // Clear index after successful commit
            index.clear();
            index.save(&sdal_root)?;
            
            println!("Created commit {} \"{}\"", &commit_hash[..7], message);
        }
        Commands::Log => {
            if !sdal_root.exists() {
                anyhow::bail!("Not an SDAL repository");
            }
            
            let storage = FilesystemStorage::new(&sdal_root)?;
            let refs = Refs::new(&sdal_root);
            
            let mut current = refs.read_head()?;
            
            if current.is_none() {
                println!("No commits yet");
                return Ok(());
            }
            
            while let Some(commit_hash) = current {
                // Skip empty hashes (shouldn't happen but prevents crashes)
                if commit_hash.is_empty() {
                    break;
                }
                
                let commit_data = storage.get(&commit_hash)?;
                let object: Object = serde_json::from_slice(&commit_data)?;
                
                if let Object::Commit(commit) = object {
                    println!("commit {}", commit_hash);
                    println!("Author: {}", commit.author);
                    println!("Date:   {}", commit.timestamp);
                    println!("\n    {}\n", commit.message);
                    
                    current = commit.parent;
                } else {
                    break;
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
            
            println!("On branch main\n");
            
            if refs.read_head()?.is_none() {
                println!("No commits yet\n");
            }
            
            // Show staged files
            if !index.entries.is_empty() {
                println!("Changes to be committed:");
                println!("  (use \"sdal reset ...\" to unstage)\n");
                for (path, _) in &index.entries {
                    println!("\t\x1b[32mnew file:   {}\x1b[0m", path);
                }
                println!();
            }
            
            // Find untracked and modified files
            let mut all_files = Vec::new();
            collect_files(&current_dir, &current_dir, &ignore, &mut all_files)?;
            
            // Get files from HEAD commit if it exists
            let mut head_files = std::collections::HashMap::new();
            if let Some(head_hash) = refs.read_head()? {
                let storage = FilesystemStorage::new(&sdal_root)?;
                if let Ok(commit_data) = storage.get(&head_hash) {
                    if let Ok(Object::Commit(commit)) = serde_json::from_slice::<Object>(&commit_data) {
                        if let Ok(tree_data) = storage.get(&commit.tree) {
                            if let Ok(Object::Tree(tree)) = serde_json::from_slice::<Object>(&tree_data) {
                                for (name, entry) in tree.entries {
                                    if let TreeEntry::Blob { hash, .. } = entry {
                                        head_files.insert(name, hash);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            let mut untracked = Vec::new();
            let mut modified = Vec::new();
            
            for (path, rel_path) in &all_files {
                if index.is_staged(&rel_path) {
                    // Already staged, skip
                    continue;
                }
                
                if let Some(head_hash) = head_files.get(rel_path) {
                    // File exists in HEAD - check if modified
                    let data = fs::read(path)?;
                    let chunker = FixedSizeChunker::new(1024 * 1024);
                    let chunks = chunker.chunk(&data)?;
                    
                    let mut chunk_entries = Vec::new();
                    for chunk in chunks {
                        chunk_entries.push(ChunkEntry::from(&chunk));
                    }
                    
                    let blob = Blob {
                        chunks: chunk_entries,
                        total_size: data.len() as u64,
                    };
                    let object = Object::Blob(blob);
                    let blob_json = serde_json::to_vec(&object)?;
                    
                    let mut hasher = Sha256::new();
                    hasher.update(&blob_json);
                    let current_hash = hex::encode(hasher.finalize());
                    
                    if &current_hash != head_hash {
                        modified.push(rel_path.clone());
                    }
                } else {
                    // File not in HEAD - it's untracked
                    untracked.push(rel_path.clone());
                }
            }
            
            // Check for deleted files (in HEAD but not in working dir)
            let working_files: std::collections::HashSet<_> = 
                all_files.iter().map(|(_, rel_path)| rel_path.as_str()).collect();
            
            let mut deleted = Vec::new();
            for (path, _) in &head_files {
                if !working_files.contains(path.as_str()) && !index.is_staged(path) {
                    deleted.push(path.clone());
                }
            }
            
            if !modified.is_empty() || !deleted.is_empty() {
                println!("Changes not staged for commit:");
                println!("  (use \"sdal add <file>...\" to update what will be committed)\n");
                for path in &modified {
                    println!("\t\x1b[33mmodified:   {}\x1b[0m", path);
                }
                for path in &deleted {
                    println!("\t\x1b[31mdeleted:    {}\x1b[0m", path);
                }
                println!();
            }
            
            if !untracked.is_empty() {
                println!("Untracked files:");
                println!("  (use \"sdal add <file>...\" to include in what will be committed)\n");
                for path in &untracked {
                    println!("\t\x1b[31m{}\x1b[0m", path);
                }
                println!();
            }
            
            if index.entries.is_empty() && untracked.is_empty() && modified.is_empty() && deleted.is_empty() {
                println!("nothing to commit, working tree clean");
            }
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
        
        let rel_path = path.strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        
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
