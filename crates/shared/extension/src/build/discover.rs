//! Discovery of an extension crate's `schema/migrations` directory: which
//! files claim which version slots, and the leading directives each up
//! migration declares.
//!
//! A panic is the only way a build script aborts the build, so a file that is
//! not named `NNN_<name>.sql` or `NNN[-MMM]_<name>.tombstone`, an orphan
//! `.down.sql`, a descending range, or a malformed `@supersedes-checksum` or
//! `@cost` directive panics here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::DiscoveredMigration;

pub(super) fn discover(dir: &Path) -> Vec<DiscoveredMigration> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut ups: Vec<PathBuf> = Vec::new();
    let mut tombstones: Vec<PathBuf> = Vec::new();
    let mut downs: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();

    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read migrations directory {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("failed to read entry in {}: {e}", dir.display()))
            .path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("tombstone") => tombstones.push(path),
            Some("sql") => {
                let stem = file_stem(&path);
                if let Some(base) = stem.strip_suffix(".down") {
                    downs.insert(base.to_owned(), path);
                } else {
                    ups.push(path);
                }
            },
            _ => {},
        }
    }

    let mut migrations: Vec<DiscoveredMigration> = ups
        .iter()
        .map(|up| {
            let stem = file_stem(up);
            let (version, end_version, name) = parse_stem(&stem, up);
            reject_malformed_cost_directive(up);
            assert!(
                version == end_version,
                "migration file {} may not name a version range — only a `.tombstone` covers \
                 more than one slot",
                up.display()
            );
            DiscoveredMigration {
                version,
                end_version,
                name,
                down_path: downs.remove(&stem),
                no_transaction: has_no_transaction_directive(up),
                supersedes: supersedes_directive(up),
                up_path: Some(up.clone()),
            }
        })
        .collect();

    if let Some((orphan_stem, orphan_path)) = downs.into_iter().next() {
        panic!(
            "down migration {} has no matching up migration {orphan_stem}.sql",
            orphan_path.display()
        );
    }

    migrations.extend(tombstones.iter().map(|path| {
        let stem = file_stem(path);
        let (version, end_version, name) = parse_stem(&stem, path);
        DiscoveredMigration {
            version,
            end_version,
            name,
            down_path: None,
            no_transaction: false,
            supersedes: Vec::new(),
            up_path: None,
        }
    }));

    migrations
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| panic!("migration path {} has no usable file stem", path.display()))
        .to_owned()
}

// Why: returns an inclusive version span. Everything but a `.tombstone` is
// asserted by the caller to span exactly one version.
fn parse_stem(stem: &str, path: &Path) -> (u32, u32, String) {
    let (prefix, name) = stem.split_once('_').unwrap_or_else(|| {
        panic!(
            "migration file {} must be named NNN_<name>.sql or NNN[-MMM]_<name>.tombstone",
            path.display()
        )
    });
    let (start_text, end_text) = prefix.split_once('-').unwrap_or((prefix, prefix));
    let start = parse_version(start_text, prefix, path);
    let end = parse_version(end_text, prefix, path);
    assert!(
        start <= end,
        "migration file {} names a descending version range `{prefix}`",
        path.display()
    );
    (start, end, name.to_owned())
}

fn parse_version(text: &str, prefix: &str, path: &Path) -> u32 {
    text.parse::<u32>().unwrap_or_else(|_| {
        panic!(
            "migration file {} has a non-numeric version prefix `{prefix}`",
            path.display()
        )
    })
}

// Why: the directive must sit in the leading comment block, before any SQL,
// so it is not confused with a statement-level comment.
fn supersedes_directive(path: &Path) -> Vec<String> {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read migration {}: {e}", path.display()));
    content
        .lines()
        .map(str::trim)
        .take_while(|line| line.is_empty() || line.starts_with("--"))
        .filter_map(|line| line.strip_prefix("-- @supersedes-checksum:"))
        .map(str::trim)
        .map(|old| {
            assert!(
                old.len() == 16 && old.chars().all(|c| c.is_ascii_hexdigit()),
                "migration {}: `@supersedes-checksum` must name a 16-hex-digit checksum, got \
                 `{old}`",
                path.display()
            );
            old.to_owned()
        })
        .collect()
}

// Why: the directive is load-bearing at runtime — the runner derives this
// migration's statement timeout from `measured` — so a malformed one must
// stop the build, exactly as a malformed `@supersedes-checksum` does, rather
// than be silently ignored and leave the migration unbounded.
fn reject_malformed_cost_directive(path: &Path) {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read migration {}: {e}", path.display()));
    if let Err(e) = crate::cost::parse(&content) {
        panic!("migration {}: {e}", path.display());
    }
}

fn has_no_transaction_directive(path: &Path) -> bool {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read migration {}: {e}", path.display()));
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line == "-- @no-transaction")
}
