// crates/policy/src/lib.rs
//
// Policy layer for SDAL.
//
// Enforces:
//   - Authentication (who is this user?)
//   - Authorization (can this user push/pull to this branch?)
//   - Access rules per branch
//
// The policy layer sits between identity verification and the protocol layer.
// The server calls policy checks AFTER verifying the Ed25519 signature
// and BEFORE handing off to protocol::handle_push / handle_fetch.

use serde::{Deserialize, Serialize};

/// Actions a user can attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Read,
    Write,
}

/// Policy check result
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

/// Check whether a user (identified by their public key hex) is allowed
/// to perform the given action on the given branch.
///
/// Phase 1: allow everything (open policy).
/// Phase 5: read from a policy store on disk.
pub fn check_access(
    _public_key_hex: &str,
    _branch: &str,
    _action: Action,
) -> PolicyDecision {
    // Phase 1: open policy — allow all authenticated requests
    PolicyDecision {
        allowed: true,
        reason: "open policy (Phase 1)".to_string(),
    }
}

/// Check whether a user can push to a specific branch.
pub fn can_push(public_key_hex: &str, branch: &str) -> PolicyDecision {
    check_access(public_key_hex, branch, Action::Write)
}

/// Check whether a user can read / fetch from a specific branch.
pub fn can_read(public_key_hex: &str, branch: &str) -> PolicyDecision {
    check_access(public_key_hex, branch, Action::Read)
}

/// Check whether a user can merge into a branch.
pub fn can_merge(public_key_hex: &str, branch: &str) -> PolicyDecision {
    // Merge requires write access
    check_access(public_key_hex, branch, Action::Write)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_policy_allows_everything() {
        let decision = can_push("abcdef1234", "main");
        assert!(decision.allowed);

        let decision = can_read("abcdef1234", "feature-x");
        assert!(decision.allowed);

        let decision = can_merge("abcdef1234", "main");
        assert!(decision.allowed);
    }
}
