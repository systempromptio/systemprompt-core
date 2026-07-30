use systemprompt_bridge::cli::doctor::{Status, filesystem};

fn valid_status(status: Status) -> bool {
    matches!(status, Status::Ok | Status::Warn | Status::Fail)
}

#[test]
fn check_bridge_working_dir_reports_writable() {
    let state = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    temp_env::with_vars(
        [
            ("XDG_STATE_HOME", Some(state.path().to_str().unwrap())),
            ("HOME", Some(home.path().to_str().unwrap())),
        ],
        || {
            let c = filesystem::check_bridge_working_dir();
            assert_eq!(c.status, Status::Ok);
            assert!(!c.name.is_empty());
            assert!(c.detail.contains("writable"), "detail = {}", c.detail);
        },
    );
}

#[test]
fn check_org_plugins_writable_returns_check() {
    let data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    temp_env::with_vars(
        [
            ("XDG_DATA_HOME", Some(data.path().to_str().unwrap())),
            ("HOME", Some(home.path().to_str().unwrap())),
        ],
        || {
            let c = filesystem::check_org_plugins_writable();
            assert!(valid_status(c.status));
            assert!(!c.name.is_empty());
            assert!(!c.detail.is_empty());
        },
    );
}
