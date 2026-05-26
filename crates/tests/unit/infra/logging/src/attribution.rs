//! Tests for the logging attribution `OnceLock`.
//!
//! `install_log_attribution` is process-wide and write-once, so most behaviour
//! is exercised indirectly: a fresh test binary can install once and observe
//! the value; subsequent installs are no-ops (the argument is dropped).

use systemprompt_identifiers::UserId;
use systemprompt_logging::{LogAttributionUnset, install_log_attribution, platform_attribution};
use systemprompt_models::services::SystemAdmin;

fn make_admin(id: &str) -> SystemAdmin {
    SystemAdmin::new(UserId::new(id), format!("user-{id}"))
}

#[test]
fn unset_before_install_returns_error_or_value() {
    // Another test may have already installed the OnceLock. Both outcomes are
    // valid; we only assert that the error variant prints sensibly when present.
    match platform_attribution() {
        Ok(admin) => {
            assert!(!admin.id().as_str().is_empty());
        },
        Err(err) => {
            let display = format!("{err}");
            assert!(display.contains("log attribution"));
            let debug = format!("{err:?}");
            assert!(debug.contains("LogAttributionUnset"));
        },
    }
}

#[test]
fn install_returns_the_active_value() {
    // Install (may be a no-op if a previous test already populated the cell).
    let installed = install_log_attribution(make_admin("test-platform-owner"));
    // We never assert which admin "won", only that some non-empty id was kept.
    assert!(!installed.id().as_str().is_empty());

    let active = platform_attribution().expect("attribution installed by now");
    assert_eq!(installed.id(), active.id());
}

#[test]
fn install_twice_is_idempotent_and_drops_argument() {
    let _first = install_log_attribution(make_admin("first-owner"));
    let second = install_log_attribution(make_admin("second-owner"));
    let current = platform_attribution().expect("installed");
    // The second call must return the originally installed admin.
    assert_eq!(second.id(), current.id());
}

#[test]
fn unset_error_is_clone_copy() {
    fn assert_clone_copy<T: Clone + Copy>() {}
    assert_clone_copy::<LogAttributionUnset>();
}
