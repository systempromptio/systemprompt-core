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
/// directory is not named `NNN_<name>.sql`, or if two files share a version.
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
    name: String,
    up_path: PathBuf,
    down_path: Option<PathBuf>,
    no_transaction: bool,
}

impl DiscoveredMigration {
    fn render(&self) -> String {
        let up = path_literal(&self.up_path);
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
}

fn discover(dir: &Path) -> Vec<DiscoveredMigration> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut ups: Vec<PathBuf> = Vec::new();
    let mut downs: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();

    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read migrations directory {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("failed to read entry in {}: {e}", dir.display()))
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let stem = file_stem(&path);
        if let Some(base) = stem.strip_suffix(".down") {
            downs.insert(base.to_owned(), path);
        } else {
            ups.push(path);
        }
    }

    let migrations: Vec<DiscoveredMigration> = ups
        .iter()
        .map(|up| {
            let stem = file_stem(up);
            let (version, name) = parse_stem(&stem, up);
            DiscoveredMigration {
                version,
                name,
                down_path: downs.remove(&stem),
                no_transaction: has_no_transaction_directive(up),
                up_path: up.clone(),
            }
        })
        .collect();

    if let Some((orphan_stem, orphan_path)) = downs.into_iter().next() {
        panic!(
            "down migration {} has no matching up migration {orphan_stem}.sql",
            orphan_path.display()
        );
    }

    migrations
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_else(|| panic!("migration path {} has no usable file stem", path.display())).to_owned()
}

fn parse_stem(stem: &str, path: &Path) -> (u32, String) {
    let (prefix, name) = stem.split_once('_').unwrap_or_else(|| {
        panic!(
            "migration file {} must be named NNN_<name>.sql",
            path.display()
        )
    });
    let version = prefix.parse::<u32>().unwrap_or_else(|_| {
        panic!(
            "migration file {} has a non-numeric version prefix `{prefix}`",
            path.display()
        )
    });
    (version, name.to_owned())
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

fn reject_duplicate_versions(migrations: &[DiscoveredMigration]) {
    for pair in migrations.windows(2) {
        if pair[0].version == pair[1].version {
            panic!(
                "two migration files share version {:03}: `{}` and `{}`",
                pair[0].version, pair[0].name, pair[1].name
            );
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
