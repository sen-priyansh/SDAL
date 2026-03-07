/// Invariant checking utilities for SDAL
/// 
/// This module provides macros and functions to enforce critical invariants
/// throughout the system. Violations should result in immediate failure.

/// Assert an invariant that must hold true
/// Panics with a descriptive message if the condition is false
#[macro_export]
macro_rules! assert_invariant {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            panic!("INVARIANT VIOLATION: {}", $msg);
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !$cond {
            panic!("INVARIANT VIOLATION: {}", format!($fmt, $($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "INVARIANT VIOLATION")]
    fn test_assert_invariant_fails() {
        assert_invariant!(false, "This should fail");
    }
    
    #[test]
    fn test_assert_invariant_passes() {
        assert_invariant!(true, "This should pass");
    }
}
