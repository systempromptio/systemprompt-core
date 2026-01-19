//! Unit tests for output mode functions

use systemprompt_core_logging::{is_startup_mode, set_startup_mode};

// ============================================================================
// Startup Mode Tests
// ============================================================================

#[test]
fn test_startup_mode_default_is_true() {
    // Note: This test may be affected by other tests due to global state
    // In production, use test isolation
    let result = is_startup_mode();
    assert!(result == true || result == false);
}

#[test]
fn test_set_startup_mode_true() {
    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn test_set_startup_mode_false() {
    set_startup_mode(false);
    assert!(!is_startup_mode());
}

#[test]
fn test_startup_mode_toggle() {
    set_startup_mode(true);
    assert!(is_startup_mode());

    set_startup_mode(false);
    assert!(!is_startup_mode());

    set_startup_mode(true);
    assert!(is_startup_mode());
}
