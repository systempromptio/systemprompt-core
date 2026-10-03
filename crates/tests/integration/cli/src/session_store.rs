use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::session::{clear_all_sessions, clear_session, load_session_store};
use tempfile::{TempDir, tempdir};

fn owned_project() -> (TempDir, ResolvedPaths) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    std::fs::create_dir_all(dir.path().join("services")).unwrap();
    let paths = ResolvedPaths::for_root(dir.path());
    (dir, paths)
}

#[test]
fn resolved_paths_resolve_under_the_project_root() {
    let (dir, paths) = owned_project();
    assert!(paths.sessions_dir().starts_with(dir.path()));
    assert!(paths.tenants_path().starts_with(dir.path()));
    assert!(paths.profiles_dir().starts_with(dir.path()));
}

#[test]
fn clear_all_sessions_creates_empty_store() {
    let (_dir, paths) = owned_project();
    clear_all_sessions(&paths).unwrap();
    assert!(load_session_store(&paths).unwrap().is_empty());
}

#[test]
fn clear_session_succeeds_with_no_profile() {
    let (_dir, paths) = owned_project();
    let _ = clear_session(&paths);
}

#[test]
fn load_session_store_in_clean_project_returns_empty() {
    let (_dir, paths) = owned_project();
    assert!(load_session_store(&paths).unwrap().is_empty());
}
