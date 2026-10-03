//! Repository-root discovery and canonical scratch directories for tests.
//!
//! The root is found by climbing until the workspace markers appear, never by
//! a fixed ancestor depth: a stale depth points at an empty directory, every
//! walk over it finds nothing, and the assertions built on it pass having
//! examined no files. Callers get a path or a panic -- never a `None` they can
//! quietly return on.
//!
//! A scratch project root comes from `canonical_tempdir`, created under the
//! canonicalised system temp directory: on macOS `/var` is a symlink to
//! `/private/var`, and code that resolves `current_dir()` sees the canonical
//! form, so a test comparing against a non-canonical tempdir path fails there.

use std::path::{Path, PathBuf};

#[must_use]
pub fn repo_root() -> PathBuf {
    find_root().unwrap_or_else(|| {
        panic!(
            "no workspace root above {}: expected an ancestor holding both Cargo.toml and crates/",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

fn find_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("crates").is_dir() && dir.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

#[must_use]
pub fn repo_path(relative: &str) -> PathBuf {
    let path = repo_root().join(relative);
    assert!(
        path.exists(),
        "{} does not exist under the workspace root",
        path.display()
    );
    path
}

#[must_use]
pub fn canonical_tempdir() -> tempfile::TempDir {
    let base = std::env::temp_dir()
        .canonicalize()
        .expect("system temp directory resolves to a canonical path");
    tempfile::tempdir_in(base).expect("create tempdir under the canonical temp directory")
}
