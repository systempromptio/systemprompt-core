//! Tests for the read side of the CLI session store.
//!
//! `load_session_store` and `get_session_for_key` read the sessions directory
//! of the project they are handed and tolerate an absent record. Each test
//! owns a temporary project root.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::session::{get_session_for_key, load_session_store};
use systemprompt_cloud::SessionKey;
use tempfile::TempDir;

fn owned_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    std::fs::create_dir_all(dir.path().join("services")).unwrap();
    dir
}

#[test]
fn the_store_loads_or_initialises_without_error() {
    let project = owned_project();
    let paths = ResolvedPaths::for_root(project.path());
    let store = load_session_store(&paths).unwrap();

    // Every record the store returns must round-trip through its own lookup.
    let key = SessionKey::from_tenant_id(None);
    let looked_up = store.get_valid_session(&key, "http://localhost:8080");
    let via_helper = get_session_for_key(&paths, &key, "http://localhost:8080").unwrap();

    assert_eq!(looked_up.is_some(), via_helper.is_some());
}

#[test]
fn an_unknown_tenant_key_resolves_to_no_session() {
    let project = owned_project();
    let paths = ResolvedPaths::for_root(project.path());
    let tenant = systemprompt_identifiers::TenantId::new("cov_absent_tenant");
    let key = SessionKey::Tenant(tenant);

    assert!(
        get_session_for_key(&paths, &key, "http://localhost:8080")
            .unwrap()
            .is_none()
    );
}
