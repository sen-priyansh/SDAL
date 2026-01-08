//crates/core/src/ignore.rs

use std::fs;
use std::path::Path;

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
                // Strip BOM if present and trim whitespace
                let line = line.trim_start_matches('\u{feff}').trim();
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
            // Require directory boundary: exact match or followed by '/'
            if path == dir_pattern || path.starts_with(&format!("{}/", dir_pattern)) {
                return true;
            }
        }

        // Wildcard pattern
        if pattern.contains('*') {
            return Self::wildcard_match(path, pattern);
        }

        // Standard directory/file match (without trailing slash in pattern)
        // Pattern "target" should match "target" and "target/foo", but NOT "target_lock"
        if path.starts_with(pattern) {
            let rest = &path[pattern.len()..];
            if rest.is_empty() || rest.starts_with('/') {
                return true;
            }
        }

        false
    }

    /// Simple wildcard matching
    fn wildcard_match(text: &str, pattern: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();

        if parts.len() == 1 {
            return text == pattern;
        }

        // For proper wildcard matching, we need to ensure all parts appear in order
        let mut remaining = text;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                // First part must be at the start
                if !remaining.starts_with(part) {
                    return false;
                }
                remaining = &remaining[part.len()..];
            } else if i == parts.len() - 1 {
                // Last part must be at the end
                if !remaining.ends_with(part) {
                    return false;
                }
            } else {
                // Middle parts must appear in order
                if let Some(pos) = remaining.find(part) {
                    remaining = &remaining[pos + part.len()..];
                } else {
                    return false;
                }
            }
        }

        true
    }
}
