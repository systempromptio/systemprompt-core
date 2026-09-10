//! Tests for `core marketplace import`.
//!
//! The command is a thin renderer over `import_anthropic_tree`, so the tests
//! drive it against the Anthropic fixture tree and assert the report reaches
//! the table and that a bad source is an error, not an empty table.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::core::marketplace::import::{ImportArgs, execute};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../domain/marketplace/fixtures/anthropic")
        .canonicalize()
        .expect("anthropic fixture tree is checked in")
}

fn args(from: PathBuf, into: PathBuf, dry_run: bool) -> ImportArgs {
    ImportArgs {
        from,
        into,
        dry_run,
        strict: false,
    }
}

#[test]
fn import_reports_every_imported_kind() {
    let dest = tempfile::tempdir().expect("tempdir");
    let output = execute(&args(fixture_root(), dest.path().join("services"), false))
        .expect("import succeeds against the fixture tree");

    let rendered = serde_json::to_string(output.artifact()).expect("artifact serialises");
    for kind in ["marketplaces", "plugins", "skills", "rules", "base_dirs"] {
        assert!(rendered.contains(kind), "report is missing the {kind} row");
    }
    assert!(
        dest.path().join("services/plugins/alpha-tools").is_dir(),
        "the plugin tree was not written"
    );
}

#[test]
fn dry_run_writes_nothing() {
    let dest = tempfile::tempdir().expect("tempdir");
    let into = dest.path().join("services");
    execute(&args(fixture_root(), into.clone(), true)).expect("dry run succeeds");
    assert!(!into.exists(), "dry run created {}", into.display());
}

#[test]
fn a_missing_source_tree_is_an_error() {
    let dest = tempfile::tempdir().expect("tempdir");
    let err = execute(&args(
        dest.path().join("nowhere"),
        dest.path().join("services"),
        false,
    ))
    .expect_err("a missing source tree must fail");
    assert!(
        err.to_string().contains("Failed to import"),
        "unexpected error: {err}"
    );
}
