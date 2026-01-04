use std::path::Path;
use std::fs;

/// Ignore patterns for SDAL (similar to .gitignore)
pub struct Ignore {
    patterns: Vec<String>,
}

impl Ignore {
    pub fn new() -> Self {
        Self {
            patterns: vec![".sdal".to_string()], // Always ignore .sdal
        }
    }
    
    /// Load ignore patterns from .sdalignore
    pub fn load<P: AsRef<Path>>(repo_root: P) -> Self {
        let mut ignore = Self::new();
        let ignore_path = repo_root.as_ref().join(".sdalignore");
        
        if let Ok(content) = fs::read_to_string(ignore_path) {
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    ignore.patterns.push(line.to_string());
                }
            }
        }
        
        ignore
    }
    
    /// Check if a path should be ignored
    pub fn should_ignore(&self, path: &str) -> bool {
        for pattern in &self.patterns {
            if Self::matches_pattern(path, pattern) {
                return true;
            }
        }
        false
    }
    
    /// Simple pattern matching (supports * and directory patterns)
    fn matches_pattern(path: &str, pattern: &str) -> bool {
        // Exact match
        if path == pattern {
            return true;
        }
        
        // Directory pattern (ends with /)
        if pattern.ends_with('/') {
            let dir_pattern = pattern.trim_end_matches('/');
            if path.starts_with(dir_pattern) {
                return true;
            }
        }
        
        // Wildcard pattern
        if pattern.contains('*') {
            return Self::wildcard_match(path, pattern);
        }
        
        // Starts with pattern
        if path.starts_with(pattern) {
            return true;
        }
        
        false
    }
    
    /// Simple wildcard matching
    fn wildcard_match(text: &str, pattern: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();
        
        if parts.len() == 1 {
            return text == pattern;
        }
        
        let first = parts[0];
        let last = parts[parts.len() - 1];
        
        if !first.is_empty() && !text.starts_with(first) {
            return false;
        }
        
        if !last.is_empty() && !text.ends_with(last) {
            return false;
        }
        
        true
    }
}
