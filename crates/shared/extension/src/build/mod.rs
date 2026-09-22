//! Build-script support for extension crates.
//!
//! [`emit_migrations`] discovers an extension crate's
//! `schema/migrations/NNN_<name>.sql` files and writes the body of
//! [`Extension::migrations`](crate::Extension) to `OUT_DIR` for
//! [`extension_migrations!`](crate::extension_migrations). Filenames define
//! versions and names, while `cargo:rerun-if-changed` tracks additions.
//!
//! # Conventions
//!
//! - `NNN_<name>.sql` — an up migration; `NNN` parses to the version, the
//!   remainder is the name.
//! - `NNN_<name>.down.sql` — the paired down migration (optional).
//! - A migration whose first non-blank line is `-- @no-transaction` is emitted
//!   with [`Migration::new_no_transaction`](crate::Migration::new_no_transaction).
//! - Each leading `-- @supersedes-checksum: <16 hex>` names replaced migration
//!   text. Its tracking row moves to the new checksum without rerunning SQL.
//!   This corrects text only when the old and new forms produce the same state;
//!   changes that must execute on established databases require a new
//!   migration.
//! - A leading `-- @cost: rows=<n> measured=<dur> triggers=<suspended|live>`
//!   states what the author measured for a migration that rewrites a hot table.
//!   The runner derives that migration's `statement_timeout` from `measured`,
//!   so a malformed directive fails the build rather than leaving the migration
//!   unbounded. See [`crate::cost`].
//! - `NNN_<name>.tombstone` / `NNN-MMM_<name>.tombstone` — a spent slot. The
//!   migration once lived here, shipped, and its file has since been deleted;
//!   established databases still carry its tracking row. A tombstone declares
//!   the number so it can never be refilled, and its body is prose, never SQL.
//!
//! Tombstones make `ls` authoritative: `reject_duplicate_versions` treats their
//! ranges as occupied, so refilling a spent slot fails the build.
//!
//! [`emit_migrations`] panics when invoked outside a build script, when a file
//! in the migrations directory is not named `NNN_<name>.sql` or
//! `NNN[-MMM]_<name>.tombstone`, or when two files claim the same version: a
//! panic is the only way a build script aborts the build.

mod discover;

use std::path::{Path, PathBuf};

use self::discover::discover;

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
    supersedes: Vec<String>,
}

impl DiscoveredMigration {
    fn render(&self) -> String {
        let Some(up_path) = self.up_path.as_ref() else {
            return self.render_tombstone();
        };
        let up = path_literal(up_path);
        let mut supersedes = String::new();
        for old in &self.supersedes {
            supersedes.push_str(&format!(".superseding({old:?})"));
        }
        match (&self.down_path, self.no_transaction) {
            (Some(_), true) => panic!(
                "migration {:03} ({}): a `-- @no-transaction` migration cannot declare a \
                 `.down.sql` — down migrations run inside a transaction",
                self.version, self.name
            ),
            (Some(down), false) => format!(
                "    ::systemprompt_extension::Migration::with_down({}, {:?}, include_str!({up}), \
                 include_str!({})){supersedes},\n",
                self.version,
                self.name,
                path_literal(down),
            ),
            (None, true) => format!(
                "    ::systemprompt_extension::Migration::new_no_transaction({}, {:?}, \
                 include_str!({up})){supersedes},\n",
                self.version, self.name,
            ),
            (None, false) => format!(
                "    ::systemprompt_extension::Migration::new({}, {:?}, \
                 include_str!({up})){supersedes},\n",
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
        (self.version..=self.end_version).fold(String::new(), |mut out, version| {
            let name = &self.name;
            out.push_str(&format!(
                "    ::systemprompt_extension::Migration::tombstone({version}, {name:?}),\n"
            ));
            out
        })
    }
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
