//! Tests for `systemprompt_extension::build::emit_migrations`.
//!
//! `emit_migrations` is the only public entry point of the build module, but it
//! drives every private helper (`discover`, `parse_stem`, `render`,
//! `has_no_transaction_directive`, `reject_duplicate_versions`, ...). It reads
//! `CARGO_MANIFEST_DIR` and writes to `OUT_DIR`. Because those env vars are
//! process-global, the scenarios are sequenced inside a single test so they do
//! not race with one another or with the cargo-provided values.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use tempfile::TempDir;

// Serializes every test in this module: they all mutate the process-global
// `CARGO_MANIFEST_DIR`/`OUT_DIR` env vars, so they must never run in parallel.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct EnvGuard {
    manifest: Option<String>,
    out: Option<String>,
}

impl EnvGuard {
    fn capture() -> Self {
        Self {
            manifest: std::env::var("CARGO_MANIFEST_DIR").ok(),
            out: std::env::var("OUT_DIR").ok(),
        }
    }

    fn set(manifest: &Path, out: &Path) {
        unsafe {
            std::env::set_var("CARGO_MANIFEST_DIR", manifest);
            std::env::set_var("OUT_DIR", out);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.manifest {
                Some(v) => std::env::set_var("CARGO_MANIFEST_DIR", v),
                None => std::env::remove_var("CARGO_MANIFEST_DIR"),
            }
            match &self.out {
                Some(v) => std::env::set_var("OUT_DIR", v),
                None => std::env::remove_var("OUT_DIR"),
            }
        }
    }
}

// Lay out a `schema/migrations` directory under `manifest`, run
// `emit_migrations`, and return the generated `migrations.rs` body.
fn run(manifest: &TempDir, out: &TempDir, files: &[(&str, &str)]) -> String {
    let migrations_dir = manifest.path().join("schema/migrations");
    fs::create_dir_all(&migrations_dir).expect("create migrations dir");
    for (name, body) in files {
        fs::write(migrations_dir.join(name), body).expect("write migration file");
    }
    EnvGuard::set(manifest.path(), out.path());
    systemprompt_extension::build::emit_migrations();
    fs::read_to_string(out.path().join("migrations.rs")).expect("generated body present")
}

#[test]
fn emit_migrations_scenarios() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();

    // --- empty / missing migrations directory yields an empty vec! body ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        // `schema/migrations` is never created.
        EnvGuard::set(manifest.path(), out.path());
        systemprompt_extension::build::emit_migrations();
        let body =
            fs::read_to_string(out.path().join("migrations.rs")).expect("body for empty dir");
        assert_eq!(
            body, "vec![\n]\n",
            "empty dir should emit an empty vec body"
        );
    }

    // --- a single plain up migration uses Migration::new and sorts by version ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[
                ("002_add_index.sql", "CREATE INDEX foo ON bar (baz);"),
                ("001_create_table.sql", "CREATE TABLE bar (id TEXT);"),
            ],
        );
        let new_one = body
            .find("Migration::new(1, \"create_table\"")
            .expect("v1 present");
        let new_two = body
            .find("Migration::new(2, \"add_index\"")
            .expect("v2 present");
        assert!(
            new_one < new_two,
            "migrations must be emitted version-ordered"
        );
        assert!(
            body.contains("include_str!"),
            "up SQL referenced via include_str!"
        );
        assert!(
            !body.contains("with_down") && !body.contains("new_no_transaction"),
            "plain migrations use neither down nor no-transaction constructors"
        );
    }

    // --- a paired down migration uses Migration::with_down ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[
                ("003_things.sql", "CREATE TABLE things (id TEXT);"),
                ("003_things.down.sql", "DROP TABLE things;"),
            ],
        );
        assert!(
            body.contains("Migration::with_down(3, \"things\""),
            "paired down migration should use with_down, got:\n{body}"
        );
        let include_count = body.matches("include_str!").count();
        assert_eq!(
            include_count, 2,
            "with_down references both up and down SQL"
        );
    }

    // --- a `-- @no-transaction` directive uses new_no_transaction ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[(
                "004_concurrent.sql",
                "-- @no-transaction\nCREATE INDEX CONCURRENTLY i ON t (c);",
            )],
        );
        assert!(
            body.contains("Migration::new_no_transaction(4, \"concurrent\""),
            "no-transaction directive should select new_no_transaction, got:\n{body}"
        );
    }

    // --- a leading blank line before the directive is still honoured ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[(
                "005_blank_then_directive.sql",
                "\n   \n-- @no-transaction\nCREATE INDEX CONCURRENTLY j ON t (c);",
            )],
        );
        assert!(
            body.contains("Migration::new_no_transaction(5, \"blank_then_directive\""),
            "first non-blank line drives the directive check, got:\n{body}"
        );
    }

    // --- a directive not on the first non-blank line is ignored (plain new) ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[(
                "006_directive_too_late.sql",
                "CREATE TABLE x (id TEXT);\n-- @no-transaction",
            )],
        );
        assert!(
            body.contains("Migration::new(6, \"directive_too_late\""),
            "directive after SQL is not honoured, got:\n{body}"
        );
    }

    // --- non-.sql files in the directory are ignored ---
    {
        let manifest = TempDir::new().expect("manifest tmp");
        let out = TempDir::new().expect("out tmp");
        let body = run(
            &manifest,
            &out,
            &[
                ("007_real.sql", "CREATE TABLE r (id TEXT);"),
                ("README.md", "not a migration"),
            ],
        );
        assert_eq!(
            body.matches("Migration::").count(),
            1,
            "only the .sql file should produce a migration, got:\n{body}"
        );
    }
}

#[test]
#[should_panic(expected = "must be called from a build script")]
fn emit_migrations_panics_without_manifest_env() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    unsafe {
        std::env::remove_var("CARGO_MANIFEST_DIR");
    }
    systemprompt_extension::build::emit_migrations();
}

#[test]
#[should_panic(expected = "NNN_<name>.sql")]
fn emit_migrations_panics_on_unprefixed_name() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    let manifest = TempDir::new().expect("manifest tmp");
    let out = TempDir::new().expect("out tmp");
    run(&manifest, &out, &[("badname.sql", "SELECT 1;")]);
}

#[test]
#[should_panic(expected = "non-numeric version prefix")]
fn emit_migrations_panics_on_non_numeric_prefix() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    let manifest = TempDir::new().expect("manifest tmp");
    let out = TempDir::new().expect("out tmp");
    run(&manifest, &out, &[("abc_name.sql", "SELECT 1;")]);
}

#[test]
#[should_panic(expected = "share version")]
fn emit_migrations_panics_on_duplicate_version() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    let manifest = TempDir::new().expect("manifest tmp");
    let out = TempDir::new().expect("out tmp");
    run(
        &manifest,
        &out,
        &[("010_one.sql", "SELECT 1;"), ("010_two.sql", "SELECT 2;")],
    );
}

#[test]
#[should_panic(expected = "no matching up migration")]
fn emit_migrations_panics_on_orphan_down() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    let manifest = TempDir::new().expect("manifest tmp");
    let out = TempDir::new().expect("out tmp");
    run(
        &manifest,
        &out,
        &[("011_ghost.down.sql", "DROP TABLE ghost;")],
    );
}

#[test]
#[should_panic(expected = "cannot declare a")]
fn emit_migrations_panics_on_no_transaction_with_down() {
    let _lock = env_lock();
    let _guard = EnvGuard::capture();
    let manifest = TempDir::new().expect("manifest tmp");
    let out = TempDir::new().expect("out tmp");
    run(
        &manifest,
        &out,
        &[
            (
                "012_idx.sql",
                "-- @no-transaction\nCREATE INDEX CONCURRENTLY k ON t (c);",
            ),
            ("012_idx.down.sql", "DROP INDEX k;"),
        ],
    );
}
