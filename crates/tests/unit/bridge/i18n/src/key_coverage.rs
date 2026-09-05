use std::path::{Path, PathBuf};
use systemprompt_bridge::i18n::t;
use systemprompt_test_fixtures::repo_path;

// Why: a fixed ancestor depth points at a different directory the moment a
// crate moves, and every walk over the wrong directory finds no files and
// passes. `repo_path` climbs to the workspace root and fails outright when the
// sources are absent, so a run that examined nothing cannot report green.
fn bridge_src() -> PathBuf {
    repo_path("bin/bridge/src")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn referenced_keys(source: &str) -> Vec<String> {
    let mut keys = Vec::new();
    // Only the fully-qualified forms are scanned: a bare `t("` also matches
    // `insert("`, `get("`, and every other identifier ending in `t`.
    for call in ["i18n::t(\"", "i18n::t_args(\""] {
        let mut rest = source;
        while let Some(at) = rest.find(call) {
            rest = &rest[at + call.len()..];
            if let Some(end) = rest.find('"') {
                let key = &rest[..end];
                if !key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    keys.push(key.to_owned());
                }
            }
        }
    }
    keys
}

#[test]
fn every_key_the_bridge_asks_for_exists_in_the_catalog() {
    let src = bridge_src();
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty(), "no bridge sources found under {src:?}");

    let mut missing: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for key in referenced_keys(&text) {
            if t(&key) == key {
                missing.push(format!("{key} ({})", file.display()));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these keys resolve to their own name, so users see the raw key: {missing:#?}"
    );
}
