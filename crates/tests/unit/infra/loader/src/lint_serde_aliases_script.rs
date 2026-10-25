//! `scripts/lint-serde-aliases.sh` with an empty allowlist: bash 5.1/5.2 abort
//! on `${#allowed[@]}` for an undeclared-empty associative array, so a tree
//! with no aliases at all (an install) used to fail the gate with no finding.

use std::path::{Path, PathBuf};
use std::process::Command;

fn core_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

#[test]
fn an_empty_allowlist_over_a_clean_tree_passes() {
    let work = tempfile::tempdir().expect("tempdir");
    let allowlist = work.path().join("allowlist.txt");
    std::fs::write(&allowlist, "").expect("write allowlist");
    let src = work.path().join("src");
    std::fs::create_dir_all(&src).expect("mkdir");
    std::fs::write(src.join("lib.rs"), "pub fn nothing() {}\n").expect("write source");

    let output = Command::new("bash")
        .arg(core_root().join("scripts/lint-serde-aliases.sh"))
        .arg("--allowlist")
        .arg(&allowlist)
        .arg(&src)
        .output()
        .expect("run the script");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 allowlisted"));
}
