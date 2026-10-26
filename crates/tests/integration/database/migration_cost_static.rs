//! Static gate: a migration that rewrites a hot table must state what it
//! measured. Pure — no database.
//!
//! The failure this prevents is not a slow query. Migrations are awaited
//! before the HTTP listener is bound, so an unmeasured backfill is an
//! instance that never opens its port, and a per-row trigger can stretch a
//! few thousand rows into minutes. The author is the only one
//! who can measure that, and `-- @cost:` is where they write it down.
//!
//! Migrations already applied to established databases cannot be annotated —
//! the checksum is taken over the whole body, so adding a comment makes every
//! established database refuse to boot. They are listed in
//! `migration_cost_baseline.txt` instead. That list may only shrink: an entry
//! whose file is gone is itself a failure, so it cannot quietly rot, and a
//! new migration can never be added to it because the gate checks the file
//! exists *and* the body is unannotated.
//!
//! The second gate here is the data paradigm's: past each extension's floor
//! in `migration_backfill_floor.txt`, a migration is DDL plus a keys-only
//! `INSERT INTO projection_backlog … SELECT`. Anything it would compute
//! belongs in the base schema, where a fresh install (which stamps
//! migrations and never runs them) gets it too.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::ExtensionId;

use systemprompt_database::{HOT_TABLES, audit_one, backfill_statements, projection_tables};

const BASELINE: &str = "migration_cost_baseline.txt";
const BACKFILL_FLOOR: &str = "migration_backfill_floor.txt";

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

fn base_schema_sql() -> Vec<String> {
    let root = repo_root();
    let mut dirs = Vec::new();
    migration_dirs(&root.join("crates"), &mut dirs);
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .filter_map(|dir| dir.parent())
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("schema readable"))
        .collect()
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
        let Some(cost) = audit_one(&ExtensionId::new("workspace"), &name, &sql, HOT_TABLES) else {
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
        let cost = audit_one(&ExtensionId::new("ext"), "001_x", sql, &hot).expect("flagged");
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
        assert!(
            audit_one(&ExtensionId::new("ext"), "001_x", sql, &hot).is_none(),
            "{sql}"
        );
    }
}

#[test]
fn a_declared_cost_satisfies_the_gate() {
    let sql =
        "-- @cost: rows=10 measured=1.5s triggers=suspended\nUPDATE ai_requests SET model = 'x';";
    let cost =
        audit_one(&ExtensionId::new("ext"), "001_x", sql, &["ai_requests"]).expect("flagged");
    assert!(!cost.is_undeclared());
    assert_eq!(cost.declared.expect("declared").rows, 10);
}

fn backfill_floor() -> std::collections::BTreeMap<String, u32> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BACKFILL_FLOOR);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must exist: {e}", path.display()));
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (extension, version) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("{BACKFILL_FLOOR}: `{line}` is not extension=NNN"));
            let version = version
                .parse::<u32>()
                .unwrap_or_else(|e| panic!("{BACKFILL_FLOOR}: `{line}`: {e}"));
            (extension.to_owned(), version)
        })
        .collect()
}

fn version_of(path: &Path) -> Option<u32> {
    path.file_stem()?
        .to_string_lossy()
        .split('_')
        .next()?
        .parse()
        .ok()
}

#[test]
fn no_backfill_after_floor() {
    let floor = backfill_floor();
    let schemas = base_schema_sql();
    let projections = projection_tables(schemas.iter().map(String::as_str));
    assert!(
        projections.contains("trace_facts"),
        "the base-schema walk did not derive trace_facts as a projection table, so the \
         cleanup allowance is reading the wrong tree: {projections:?}"
    );
    let mut offenders = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut highest = std::collections::BTreeMap::<String, u32>::new();

    for path in migration_files() {
        let name = label(&path);
        let (extension, _) = name.split_once('/').expect("label is extension/stem");
        let Some(version) = version_of(&path) else {
            continue;
        };
        let top = highest.entry(extension.to_owned()).or_default();
        *top = (*top).max(version);
        let bound = *floor
            .get(extension)
            .unwrap_or_else(|| panic!("{BACKFILL_FLOOR} has no line for extension `{extension}`"));
        seen.insert(extension.to_owned());
        if version <= bound {
            continue;
        }
        let sql = std::fs::read_to_string(&path).expect("migration readable");
        for statement in backfill_statements(&sql)
            .into_iter()
            .filter(|statement| !statement.is_projection_cleanup(&projections))
        {
            offenders.push(format!(
                "{name}: statement {} {} {}",
                statement.position, statement.form, statement.target
            ));
        }
    }

    let unknown: Vec<&String> = floor.keys().filter(|k| !seen.contains(*k)).collect();
    assert!(
        unknown.is_empty(),
        "{BACKFILL_FLOOR} names extensions with no migrations: {unknown:?}"
    );
    for (extension, bound) in &floor {
        let top = highest.get(extension).copied().unwrap_or_default();
        assert!(
            *bound <= top,
            "{BACKFILL_FLOOR}: {extension}={bound} is past its highest migration {top:03}"
        );
    }
    assert!(
        offenders.is_empty(),
        "these migrations populate data instead of queuing keys:\n  {}\n\n\
         Past the floor a migration is DDL (the same text as the base schema) plus\n  \
         INSERT INTO projection_backlog SELECT '<projection>', <key> FROM <relation> …\n\
         Put the projector, its statement trigger and the drainer in the base schema so a\n\
         fresh install, which stamps migrations without running them, gets the same facts.\n\
         The one data statement allowed is cleanup: a DELETE from projection_backlog or from a\n\
         projection's own fact table (a table the base schema creates and queues by name\n\
         through queue_projection_backlog).",
        offenders.join("\n  ")
    );
}

#[test]
fn the_backfill_detector_sees_each_populating_form() {
    let cases = [
        ("UPDATE ai_requests SET x = 1;", "UPDATE", "ai_requests"),
        ("DELETE FROM logs WHERE level = 'debug';", "DELETE", "logs"),
        (
            "INSERT INTO facts (id, n) SELECT id, COUNT(*) FROM ledger GROUP BY id;",
            "INSERT … SELECT",
            "facts",
        ),
        (
            "SELECT refresh_conversation_facts(ARRAY['a']);",
            "SELECT refresh_*()",
            "refresh_conversation_facts",
        ),
        (
            "INSERT INTO projection_backlog (projection, key) SELECT 'p', l.id FROM ledger l JOIN other o ON o.id = l.id;",
            "INSERT … SELECT",
            "projection_backlog",
        ),
    ];
    for (sql, form, target) in cases {
        let found = backfill_statements(sql);
        assert_eq!(found.len(), 1, "{sql}");
        assert_eq!(found[0].form, form, "{sql}");
        assert_eq!(found[0].target, target, "{sql}");
    }
}

#[test]
fn the_backfill_detector_allows_ddl_and_keys_only_queueing() {
    for sql in [
        "CREATE TABLE IF NOT EXISTS t (id TEXT PRIMARY KEY);",
        "ALTER TABLE t ADD COLUMN IF NOT EXISTS n INTEGER;",
        "CREATE INDEX IF NOT EXISTS i ON t(n);",
        "INSERT INTO projection_backlog (projection, key) SELECT 'conversation_facts', context_id FROM conversation_facts_backlog ON CONFLICT DO NOTHING;",
        "INSERT INTO projection_backlog (projection, key) SELECT 'p', id FROM ledger WHERE fact IS NULL;",
        "INSERT INTO projection_backlog (projection, key) SELECT DISTINCT 'p', to_char(date_trunc('hour', created_at, 'UTC'), 'YYYY-MM-DD\"T\"HH24:00:00Z') FROM ledger ON CONFLICT DO NOTHING;",
        "INSERT INTO t (id) VALUES ('x');",
        "DROP TABLE IF EXISTS old;",
        "CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE sql AS $$ UPDATE t SET n = 1 $$;",
        "SELECT queue_projection_backlog('p', ARRAY['a']);",
    ] {
        assert!(backfill_statements(sql).is_empty(), "{sql}");
    }
}

#[test]
fn projection_tables_are_the_queued_projections_the_schema_creates() {
    let schemas = [
        "CREATE TABLE IF NOT EXISTS trace_facts (trace_id TEXT PRIMARY KEY);",
        "CREATE OR REPLACE FUNCTION q() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN PERFORM queue_projection_backlog('trace_facts', ARRAY['a']); \
         PERFORM queue_projection_backlog( 'no_such_table', ARRAY['a']); RETURN NULL; END $$;",
        "CREATE TABLE IF NOT EXISTS logs (id TEXT PRIMARY KEY);",
    ];
    let tables = projection_tables(schemas);
    assert_eq!(tables.into_iter().collect::<Vec<_>>(), vec!["trace_facts"]);
}

#[test]
fn the_backfill_detector_treats_projection_deletes_as_cleanup() {
    let tables = BTreeSet::from(["trace_facts".to_owned()]);
    for sql in [
        "DELETE FROM projection_backlog b WHERE b.projection = 'trace_facts';",
        "DELETE FROM trace_facts f WHERE f.ai_count = 0;",
    ] {
        let found = backfill_statements(sql);
        assert_eq!(found.len(), 1, "{sql}");
        assert!(found[0].is_projection_cleanup(&tables), "{sql}");
    }
}

#[test]
fn the_backfill_detector_keeps_flagging_writes_that_are_not_projection_cleanup() {
    let tables = BTreeSet::from(["trace_facts".to_owned()]);
    for sql in [
        "DELETE FROM logs WHERE level = 'debug';",
        "UPDATE trace_facts SET ai_count = 0;",
        "INSERT INTO trace_facts (trace_id) SELECT trace_id FROM logs GROUP BY trace_id;",
        "UPDATE projection_backlog SET attempts = 0;",
    ] {
        let found = backfill_statements(sql);
        assert_eq!(found.len(), 1, "{sql}");
        assert!(!found[0].is_projection_cleanup(&tables), "{sql}");
    }
}
