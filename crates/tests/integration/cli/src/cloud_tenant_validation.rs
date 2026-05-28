use systemprompt_cli::cloud::tenant::{check_build_ready, find_services_config};
use tempfile::tempdir;

#[test]
fn find_services_config_missing_returns_error() {
    let root = tempdir().unwrap();
    let err = find_services_config(root.path()).expect_err("expected missing config error");
    assert!(err.to_string().contains("Services config not found"));
}

#[test]
fn find_services_config_returns_path_when_present() {
    let root = tempdir().unwrap();
    let nested = root.path().join("services").join("config");
    std::fs::create_dir_all(&nested).unwrap();
    let cfg = nested.join("config.yaml");
    std::fs::write(&cfg, "settings: {}\n").unwrap();
    let resolved = find_services_config(root.path()).expect("config should resolve");
    assert_eq!(resolved, cfg);
}

#[test]
fn check_build_ready_in_empty_dir_returns_string_error() {
    let _g = crate::env_lock::ENV
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let original = std::env::current_dir().ok();
    let root = tempdir().unwrap();
    if std::env::set_current_dir(root.path()).is_ok() {
        let result = check_build_ready();
        assert!(result.is_err());
        // Restore CWD so other tests don't see a deleted dir.
        if let Some(orig) = original {
            let _ = std::env::set_current_dir(orig);
        }
    }
}
