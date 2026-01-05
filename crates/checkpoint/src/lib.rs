use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Checkpoint {
    pub id: String,
    pub tree_root: String,      // Tree hash
    pub parent_commit: String,   // HEAD at creation
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
    
    pub fn next_id(&self) -> String {
        let num = self.checkpoints.len() + 1;
        format!("cp_{:04}", num)
    }
}

pub mod store;
pub mod ops;
