//! Unit tests for output mode functions

use std::sync::Mutex;
use systemprompt_logging::{is_startup_mode, set_startup_mode};

static STARTUP_MODE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_startup_mode_returns_bool() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let _result = is_startup_mode();
    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn test_set_startup_mode_true() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(true);
    assert!(is_startup_mode());
}

#[test]
fn test_set_startup_mode_false() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(false);
    assert!(!is_startup_mode());
}

#[test]
fn test_startup_mode_toggle() {
    let _guard = STARTUP_MODE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_startup_mode(true);
    assert!(is_startup_mode());

    set_startup_mode(false);
    assert!(!is_startup_mode());

    set_startup_mode(true);
    assert!(is_startup_mode());
}
