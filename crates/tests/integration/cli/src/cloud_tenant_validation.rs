use systemprompt_cli::cloud::tenant::check_build_ready;
use tempfile::tempdir;


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
