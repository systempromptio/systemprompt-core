//! Proves every `crates/**/schema/migrations` directory numbers its slots
//! contiguously, or records the exception as a `.tombstone`.
//!
//! A deleted migration leaves a hole that `ls` cannot distinguish from a slot
//! that was never used, so the next author re-uses the number and ships a
//! migration the runtime silently refuses. The tombstone file is what makes
//! the difference visible at commit time rather than at deploy time.
//!
//! The repo root is found by climbing until `crates/` appears, never by a
//! fixed ancestor depth -- a stale depth walks an empty tree and passes. For
//! the same reason this test fails outright when it finds no migration
//! directories at all: a skipped run must not look like a green one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("crates").is_dir() && dir.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

fn migration_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.ends_with("migrations") && path.parent().is_some_and(|p| p.ends_with("schema")) {
            out.push(path);
        } else if !path.ends_with("target") {
            migration_dirs(&path, out);
        }
    }
}

fn slots(dir: &Path) -> BTreeMap<u32, String> {
    let mut found = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((prefix, _)) = name.split_once('_') else {
            continue;
        };
        if let Ok(number) = prefix.parse::<u32>() {
            found.insert(number, name);
        }
    }
    found
}

#[test]
fn every_migration_slot_is_used_or_tombstoned() {
    let root = repo_root().expect("repo root with a crates/ directory");
    let mut dirs = Vec::new();
    migration_dirs(&root.join("crates"), &mut dirs);
    assert!(
        !dirs.is_empty(),
        "found no schema/migrations directories under {}; the walk is broken, not the tree",
        root.display()
    );

    let mut gaps = Vec::new();
    for dir in &dirs {
        let found = slots(dir);
        let Some(highest) = found.keys().next_back().copied() else {
            continue;
        };
        for number in 1..=highest {
            if !found.contains_key(&number) {
                gaps.push(format!("{}: slot {number:03} is missing", dir.display()));
            }
        }
    }

    assert!(
        gaps.is_empty(),
        "migration slots with neither a .sql nor a .tombstone file:\n{}\n\nA deleted migration \
         must leave a `NNN_<name>.tombstone` behind so the slot cannot be re-used.",
        gaps.join("\n")
    );
}
