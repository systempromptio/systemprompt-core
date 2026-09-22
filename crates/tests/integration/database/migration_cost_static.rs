//! Static gate: a migration that rewrites a hot table must state what it
//! measured. Pure — no database.
//!
//! The failure this prevents is not a slow query. Migrations are awaited
//! before the HTTP listener is bound, so an unmeasured backfill is an
//! instance that never opens its port: a 3,644-row `UPDATE ai_requests` took
//! 27 minutes on a production instance because one per-row trigger
//! re-enqueued the whole client session per row. The author is the only one
//! who can measure that, and `-- @cost:` is where they write it down.
//!
//! Migrations that shipped before this gate cannot be annotated — the
//! checksum is taken over the whole body, so adding a comment makes every
//! established database refuse to boot. They are listed in
//! `migration_cost_baseline.txt` instead. That list may only shrink: an entry
//! whose file is gone is itself a failure, so it cannot quietly rot, and a
//! new migration can never be added to it because the gate checks the file
//! exists *and* the body is unannotated.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use systemprompt_database::{HOT_TABLES, audit_one};

const BASELINE: &str = "migration_cost_baseline.txt";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("crates").is_dir() && dir.join("Cargo.toml").is_file())
        .expect("repo root with a crates/ directory")
        .to_path_buf()
}

// Why: climbing for the root rather than a fixed depth — a stale depth walks
// an empty tree and passes, which is how a gate stops being a gate.
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

/// `<extension>/<file stem>`, the same label the baseline records.
fn label(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path
        .ancestors()
        .nth(3)
        .and_then(Path::file_name)
        .unwrap_or_default()
        .to_string_lossy();
    format!("{extension}/{stem}")
}

fn baseline() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn migration_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut dirs = Vec::new();
    migration_dirs(&root.join("crates"), &mut dirs);
    assert!(
        !dirs.is_empty(),
        "found no migration directories under {} — a skipped run must not look green",
        root.display()
    );
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|e| e == "sql")
                && !path.to_string_lossy().ends_with(".down.sql")
        })
        .collect();
    files.sort();
    files
}

#[test]
fn every_hot_table_rewrite_declares_its_measured_cost() {
    let allowed = baseline();
    let mut undeclared = Vec::new();
    let mut malformed = Vec::new();
    let mut declared = 0usize;
    let mut grandfathered = BTreeSet::new();

    for path in migration_files() {
        let sql = std::fs::read_to_string(&path).expect("migration readable");
        let name = label(&path);
        let Some(cost) = audit_one("", &name, &sql, HOT_TABLES) else {
            continue;
        };
        if let Some(reason) = cost.malformed {
            malformed.push(format!("{name}: {reason}"));
            continue;
        }
        if cost.declared.is_some() {
            declared += 1;
            continue;
        }
        if cost.is_undeclared() {
            if allowed.contains(&name) {
                grandfathered.insert(name);
            } else {
                undeclared.push(format!("{name}: {}", cost.statement_summary()));
            }
        }
    }

    assert!(
        malformed.is_empty(),
        "migrations with an unparseable `-- @cost:` directive:\n  {}",
        malformed.join("\n  ")
    );
    assert!(
        undeclared.is_empty(),
        "these migrations rewrite a hot table without declaring what it costs:\n  {}\n\n\
         Measure it against a production-shaped copy, then add a leading line:\n  \
         -- @cost: rows=<written> measured=<wall clock, e.g. 2.0s> triggers=<suspended|live>\n\n\
         If the fan-out is what makes it slow, suspend the per-row triggers around the\n\
         statement and say `triggers=suspended`; if the projections must see the change,\n\
         leave them live and say so. The runner turns `measured` into this migration's\n\
         statement_timeout, so the number is load-bearing, not a comment.",
        undeclared.join("\n  ")
    );

    let stale: Vec<&String> = allowed.difference(&grandfathered).collect();
    assert!(
        stale.is_empty(),
        "{BASELINE} names migrations that no longer need grandfathering:\n  {stale:?}\n\
         Delete those lines — the baseline may only shrink."
    );
    println!(
        "migration cost: {declared} declared, {} grandfathered",
        grandfathered.len()
    );
}

#[test]
fn the_detector_sees_each_expensive_form() {
    let hot = ["ai_requests"];
    let cases = [
        ("UPDATE ai_requests SET model = 'x';", "UPDATE on"),
        ("DELETE FROM ai_requests WHERE id = 'x';", "DELETE from"),
        (
            "INSERT INTO ai_requests (id) SELECT id FROM other;",
            "INSERT … SELECT into",
        ),
        (
            "CREATE INDEX idx_x ON ai_requests(model);",
            "CREATE INDEX (not CONCURRENTLY) on",
        ),
        (
            "ALTER TABLE ai_requests VALIDATE CONSTRAINT c;",
            "ALTER TABLE … VALIDATE CONSTRAINT on",
        ),
        (
            "ALTER TABLE ai_requests ALTER COLUMN model SET NOT NULL;",
            "ALTER TABLE … SET NOT NULL on",
        ),
    ];
    for (sql, form) in cases {
        let cost = audit_one("ext", "001_x", sql, &hot).expect("flagged");
        assert!(cost.is_undeclared(), "{sql}");
        assert_eq!(cost.statements[0].form, form, "{sql}");
        assert_eq!(cost.statements[0].table, "ai_requests", "{sql}");
    }
}

#[test]
fn the_detector_leaves_ordinary_migrations_alone() {
    let hot = ["ai_requests"];
    for sql in [
        "ALTER TABLE ai_requests ADD COLUMN IF NOT EXISTS x TEXT;",
        "CREATE INDEX CONCURRENTLY idx_x ON ai_requests(model);",
        "INSERT INTO ai_requests (id) VALUES ('x');",
        "UPDATE some_small_table SET x = 1;",
        "CREATE TABLE t (id TEXT);",
    ] {
        assert!(audit_one("ext", "001_x", sql, &hot).is_none(), "{sql}");
    }
}

#[test]
fn a_declared_cost_satisfies_the_gate() {
    let sql =
        "-- @cost: rows=10 measured=1.5s triggers=suspended\nUPDATE ai_requests SET model = 'x';";
    let cost = audit_one("ext", "001_x", sql, &["ai_requests"]).expect("flagged");
    assert!(!cost.is_undeclared());
    assert_eq!(cost.declared.expect("declared").rows, 10);
}
