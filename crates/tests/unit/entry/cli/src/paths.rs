//! Unit tests for the `paths` module.
//!
//! `ResolvedPaths::discover` walks the filesystem from the current working
//! directory looking for `.systemprompt/`. We can't predict whether that
//! finds anything in the test environment, but we can still exercise every
//! method on the resulting struct.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::paths::ResolvedPaths;

#[test]
fn discover_returns_struct_with_callable_accessors() {
    let paths = ResolvedPaths::discover();
    let _ = paths.sessions_dir();
    let _ = paths.tenants_path();
    let _ = paths.profiles_dir();
}

#[test]
fn sessions_dir_is_non_empty_path() {
    let paths = ResolvedPaths::discover();
    let sessions = paths.sessions_dir();
    assert!(sessions.ends_with("sessions"), "got: {}", sessions.display());
}

#[test]
fn tenants_path_is_non_empty_path() {
    let paths = ResolvedPaths::discover();
    let tenants = paths.tenants_path();
    assert!(tenants.ends_with("tenants.json"), "got: {}", tenants.display());
}

#[test]
fn profiles_dir_is_non_empty_path() {
    let paths = ResolvedPaths::discover();
    let profiles = paths.profiles_dir();
    assert!(profiles.ends_with("profiles"), "got: {}", profiles.display());
}

#[test]
fn discover_resolved_paths_debug() {
    let paths = ResolvedPaths::discover();
    let debug = format!("{:?}", paths);
    assert!(debug.contains("ResolvedPaths"));
}
