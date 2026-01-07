use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Checkpoint {
    pub id: String,
    pub tree_root: String,             // Tree hash
    pub parent_commit: Option<String>, // HEAD at creation (None if no commits yet)
    pub timestamp: i64,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CheckpointIndex {
    pub current: Option<String>,
    pub checkpoints: Vec<Checkpoint>,
}

impl CheckpointIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, checkpoint: Checkpoint) {
        // Prevent duplicate IDs
        assert!(
            !self.checkpoints.iter().any(|cp| cp.id == checkpoint.id),
            "Duplicate checkpoint ID: {}",
            checkpoint.id
        );

        self.current = Some(checkpoint.id.clone());
        self.checkpoints.push(checkpoint);
    }

    pub fn find(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|cp| cp.id == id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.checkpoints.iter().position(|cp| cp.id == id) {
            self.checkpoints.remove(pos);
            if self.current.as_ref() == Some(&id.to_string()) {
                self.current = self.checkpoints.last().map(|cp| cp.id.clone());
            }
            true
        } else {
            false
        }
    }

    /// Generate next checkpoint ID based on max existing ID
    /// Guarantees no reuse even after deletions
    pub fn next_id(&self) -> String {
        let max = self
            .checkpoints
            .iter()
            .filter_map(|cp| cp.id.strip_prefix("cp_"))
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);

        format!("cp_{:04}", max + 1)
    }
}

pub mod ops;
pub mod store;
