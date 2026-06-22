//! Unit tests for `ConfigValidator::check_file_permissions` — the
//! `.env` file permission audit and its warning reporting.

use systemprompt_config::{ConfigValidator, ValidationReport};

#[test]
fn check_file_permissions_noop_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("does-not-exist.env");
    let mut report = ValidationReport::new();

    ConfigValidator::check_file_permissions(&path, &mut report)
        .expect("missing file should be a no-op");
    assert!(report.warnings.is_empty(), "missing file adds no warnings");
    assert!(report.errors.is_empty());
}

#[cfg(unix)]
#[test]
fn check_file_permissions_accepts_600() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secure.env");
    std::fs::write(&path, b"PORT=8080\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let mut report = ValidationReport::new();
    ConfigValidator::check_file_permissions(&path, &mut report).expect("0600 perms ok");
    assert!(
        report.warnings.is_empty(),
        "0600 should not warn, got: {:?}",
        report.warnings
    );
}

#[cfg(unix)]
#[test]
fn check_file_permissions_accepts_644() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("readable.env");
    std::fs::write(&path, b"PORT=8080\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let mut report = ValidationReport::new();
    ConfigValidator::check_file_permissions(&path, &mut report).expect("0644 perms ok");
    assert!(
        report.warnings.is_empty(),
        "0644 should not warn, got: {:?}",
        report.warnings
    );
}

#[cfg(unix)]
#[test]
fn check_file_permissions_warns_on_world_writable() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("exposed.env");
    std::fs::write(&path, b"OAUTH_AT_REST_PEPPER=x\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

    let mut report = ValidationReport::new();
    ConfigValidator::check_file_permissions(&path, &mut report).expect("call succeeds");
    assert!(
        report.warnings.iter().any(|w| w.contains("666")),
        "0666 should warn about exposed secrets, got: {:?}",
        report.warnings
    );
    assert!(
        report.errors.is_empty(),
        "permission audit emits warnings, not errors"
    );
}
