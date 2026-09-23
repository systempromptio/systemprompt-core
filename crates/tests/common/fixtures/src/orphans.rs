//! Test resources that outlive the process that made them.
//!
//! A test's temp directory and disposable database cannot be relied on to
//! clean themselves up: the bootstrap's `TempDir` lives in a `static`, and Rust
//! never runs a static's destructor; a disposable database is dropped by an
//! async call a panicking test never reaches. Both leaked on every run — 7,975
//! directories and 607 databases had piled up locally.
//!
//! So each resource is named with the PID of the process that owns it, and
//! every new one first removes those whose owner is no longer running. That
//! also covers a run that was killed or timed out, which no `Drop` could.

use std::path::Path;

use sqlx::PgPool;

pub const TEMPDIR_PREFIX: &str = "sptest-";

const DATABASE_OWNER_MARK: &str = "_p";
const DATABASE_SUFFIX_HEX: usize = 12;
const POSTGRES_NAME_LIMIT: usize = 63;

#[must_use]
pub fn tempdir_prefix() -> String {
    format!("{TEMPDIR_PREFIX}{}-", std::process::id())
}

pub fn sweep_tempdirs() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(owner) = name
            .to_str()
            .and_then(|n| n.strip_prefix(TEMPDIR_PREFIX))
            .and_then(|rest| rest.split('-').next())
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        if !owner_is_gone(owner) {
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(entry.path()) {
            eprintln!("could not remove orphaned test directory {name:?}: {e}");
        }
    }
}

#[must_use]
pub fn database_name(prefix: &str) -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let tail = format!(
        "{DATABASE_OWNER_MARK}{}_{}",
        std::process::id(),
        &uuid[..DATABASE_SUFFIX_HEX]
    );
    let head: String = prefix
        .chars()
        .take(POSTGRES_NAME_LIMIT.saturating_sub(tail.len()))
        .collect();
    format!("{head}{tail}")
}

pub async fn sweep_databases(admin: &PgPool) {
    let pattern = format!("{DATABASE_OWNER_MARK}[0-9]+_[0-9a-f]{{{DATABASE_SUFFIX_HEX}}}$");
    let names =
        match sqlx::query_scalar::<_, String>("SELECT datname FROM pg_database WHERE datname ~ $1")
            .bind(&pattern)
            .fetch_all(admin)
            .await
        {
            Ok(names) => names,
            Err(e) => {
                eprintln!("could not list test databases to sweep: {e}");
                return;
            },
        };
    for name in names {
        let Some(owner) = database_owner(&name) else {
            continue;
        };
        if !owner_is_gone(owner) {
            continue;
        }
        if let Err(e) = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"
        )))
        .execute(admin)
        .await
        {
            eprintln!("could not drop orphaned test database {name}: {e}");
        }
    }
}

fn database_owner(name: &str) -> Option<u32> {
    let (rest, _suffix) = name.rsplit_once('_')?;
    let (_, pid) = rest.rsplit_once(DATABASE_OWNER_MARK)?;
    pid.parse().ok()
}

// Why: only Linux can answer "is this PID running" from the filesystem; the
// suites run there, locally and in CI. Anywhere else nothing counts as gone,
// so the sweep is a no-op rather than a guess that drops a live run's data.
fn owner_is_gone(pid: u32) -> bool {
    cfg!(target_os = "linux")
        && pid != std::process::id()
        && !Path::new(&format!("/proc/{pid}")).exists()
}
