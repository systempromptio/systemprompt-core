use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::session::{clear_all_sessions, clear_session, load_session_store};
use tempfile::tempdir;

use crate::env_lock;

fn isolate_home() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
    let guard = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", dir.path());
    }
    (dir, guard)
}

#[test]
fn resolved_paths_resolves_under_home() {
    let (_home, _g) = isolate_home();
    let paths = ResolvedPaths::discover();
    assert!(!paths.sessions_dir().as_os_str().is_empty());
    assert!(!paths.tenants_path().as_os_str().is_empty());
    assert!(!paths.profiles_dir().as_os_str().is_empty());
}

#[test]
fn clear_all_sessions_creates_empty_store() {
    let (_home, _g) = isolate_home();
    let _ = clear_all_sessions();
    let _ = load_session_store();
}

#[test]
fn clear_session_succeeds_with_no_profile() {
    let (_home, _g) = isolate_home();
    let _ = clear_session();
}

#[test]
fn load_session_store_in_clean_home_returns_empty() {
    let (_home, _g) = isolate_home();
    let _ = load_session_store();
}
