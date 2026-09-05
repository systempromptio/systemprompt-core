//! Build-script support for extension crates.
//!
//! Extensions keep their schema migrations as
//! `schema/migrations/NNN_<name>.sql` files. [`emit_migrations`] is called from
//! an extension crate's `build.rs`: it discovers those files, derives each
//! migration's version and name from the filename, and writes the body of
//! [`Extension::migrations`](crate::Extension) to `OUT_DIR`. The extension
//! consumes the generated body with the
//! [`extension_migrations!`](crate::extension_migrations) macro.
//!
//! Because the filename is the single source of version and name, those values
//! cannot drift from the SQL they label, and `cargo:rerun-if-changed` makes a
//! newly added file retrigger the build.
//!
//! # Conventions
//!
//! - `NNN_<name>.sql` — an up migration; `NNN` parses to the version, the
//!   remainder is the name.
//! - `NNN_<name>.down.sql` — the paired down migration (optional).
//! - A migration whose first non-blank line is `-- @no-transaction` is emitted
//!   with [`Migration::new_no_transaction`](crate::Migration::new_no_transaction).
//! - `NNN_<name>.tombstone` / `NNN-MMM_<name>.tombstone` — a spent slot. The
//!   migration once lived here, shipped, and its file has since been deleted;
//!   established databases still carry its tracking row. A tombstone declares
//!   the number so it can never be refilled, and its body is prose, never SQL.
//!
//! Tombstones are what make `ls` the truth: a number is free only if no file
//! claims it. Because [`reject_duplicate_versions`] treats a tombstoned range
//! exactly like an occupied one, refilling a spent slot fails the **build**,
//! long before a deployment discovers it as a checksum mismatch naming a
//! migration nobody recognises.

use std::path::{Path, PathBuf};

/// Generate the migration list from the conventional `schema/migrations`
/// directory and write it to `OUT_DIR/migrations.rs`.
///
/// Call from a `build.rs`:
///
/// ```rust,ignore
/// fn main() {
///     systemprompt_extension::build::emit_migrations();
/// }
/// ```
///
/// # Panics
///
/// Panics if invoked outside a build script, if a file in the migrations
/// directory is not named `NNN_<name>.sql` or `NNN[-MMM]_<name>.tombstone`, or
/// if two files claim the same version.
pub fn emit_migrations() {
    let manifest = required_env("CARGO_MANIFEST_DIR");
    let dir = Path::new(&manifest).join("schema/migrations");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut migrations = discover(&dir);
    migrations.sort_by_key(|m| m.version);
    reject_duplicate_versions(&migrations);

    let mut body = String::from("vec![\n");
    for migration in &migrations {
        body.push_str(&migration.render());
    }
    body.push_str("]\n");

    let out = PathBuf::from(required_env("OUT_DIR")).join("migrations.rs");
    if let Err(e) = std::fs::write(&out, body) {
        panic!("failed to write {}: {e}", out.display());
    }
}

struct DiscoveredMigration {
    version: u32,
    end_version: u32,
    name: String,
    up_path: Option<PathBuf>,
    down_path: Option<PathBuf>,
    no_transaction: bool,
}

impl DiscoveredMigration {
    fn render(&self) -> String {
        let Some(up_path) = self.up_path.as_ref() else {
            return self.render_tombstone();
        };
        let up = path_literal(up_path);
        match (&self.down_path, self.no_transaction) {
            (Some(_), true) => panic!(
                "migration {:03} ({}): a `-- @no-transaction` migration cannot declare a \
                 `.down.sql` — down migrations run inside a transaction",
                self.version, self.name
            ),
            (Some(down), false) => format!(
                "    ::systemprompt_extension::Migration::with_down({}, {:?}, include_str!({up}), \
                 include_str!({})),\n",
                self.version,
                self.name,
                path_literal(down),
            ),
            (None, true) => format!(
                "    ::systemprompt_extension::Migration::new_no_transaction({}, {:?}, \
                 include_str!({up})),\n",
                self.version, self.name,
            ),
            (None, false) => format!(
                "    ::systemprompt_extension::Migration::new({}, {:?}, include_str!({up})),\n",
                self.version, self.name,
            ),
        }
    }

    // Why: one entry per covered version — the runner reasons about single
    // versions, so a range is expanded here rather than understood downstream.
    fn render_tombstone(&self) -> String {
        assert!(
            self.down_path.is_none(),
            "tombstone {:03} ({}) cannot declare a `.down.sql` — a spent slot has no SQL to \
             revert",
            self.version,
            self.name
        );
        (self.version..=self.end_version)
            .map(|version| {
                format!(
                    "    ::systemprompt_extension::Migration::tombstone({}, {:?}),\n",
                    version, self.name
                )
            })
            .collect()
    }
}

fn discover(dir: &Path) -> Vec<DiscoveredMigration> {
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
            }
            _ => {}
        }
    }

    let mut migrations: Vec<DiscoveredMigration> = ups
        .iter()
        .map(|up| {
            let stem = file_stem(up);
            let (version, end_version, name) = parse_stem(&stem, up);
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

fn has_no_transaction_directive(path: &Path) -> bool {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read migration {}: {e}", path.display()));
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line == "-- @no-transaction")
}

// Why: the spans are sorted by start, so an overlap can only be with the
// immediate predecessor's high-water end. A tombstoned span counts as occupied,
// which is what turns refilling a spent slot into a build failure.
fn reject_duplicate_versions(migrations: &[DiscoveredMigration]) {
    let mut highest: Option<&DiscoveredMigration> = None;
    for migration in migrations {
        if let Some(previous) = highest
            && previous.end_version >= migration.version
        {
            panic!(
                "two migration files claim version {:03}: `{}` and `{}` — if one of them is a \
                 tombstone the slot is already spent, so take the next free number",
                migration.version, previous.name, migration.name
            );
        }
        if highest.is_none_or(|p| migration.end_version > p.end_version) {
            highest = Some(migration);
        }
    }
}

fn path_literal(path: &Path) -> String {
    let text = path
        .to_str()
        .unwrap_or_else(|| panic!("migration path {} is not valid UTF-8", path.display()));
    format!("{text:?}")
}

fn required_env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| {
        panic!("{key} is not set; this function must be called from a build script")
    })
}
