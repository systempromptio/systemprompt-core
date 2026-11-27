//! The embedded schema. Migrations are applied in order and counted in `PRAGMA
//! user_version`, so opening a database twice, or opening one another process
//! already migrated, applies nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::store::error::{Result, StoreError};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const MIGRATIONS: &[&str] = &[V1_INITIAL];

const V1_INITIAL: &str = "
CREATE TABLE project (
    id TEXT PRIMARY KEY NOT NULL,
    worktree TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE session (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES session(id) ON DELETE CASCADE,
    directory TEXT NOT NULL,
    title TEXT NOT NULL,
    agent TEXT,
    model TEXT,
    permission TEXT,
    revert TEXT,
    additions INTEGER NOT NULL DEFAULT 0,
    deletions INTEGER NOT NULL DEFAULT 0,
    files INTEGER NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);
CREATE INDEX session_project_idx ON session (project_id, parent_id, updated_at);
CREATE INDEX session_parent_idx ON session (parent_id, created_at);

CREATE TABLE message (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    agent TEXT NOT NULL,
    model TEXT NOT NULL,
    parent_id TEXT,
    is_system INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    finish TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    completed_at INTEGER
);
CREATE INDEX message_session_idx ON message (session_id, id);

CREATE TABLE part (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES message(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX part_message_idx ON part (message_id, ordinal, id);
CREATE INDEX part_session_idx ON part (session_id, message_id);

CREATE TABLE todo (
    session_id TEXT NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    content TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    PRIMARY KEY (session_id, ordinal)
) WITHOUT ROWID;
";

pub(crate) fn supported_version() -> i64 {
    i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX)
}

pub(crate) fn configure_writer(connection: &Connection) -> Result<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let _mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

pub(crate) fn configure_reader(connection: &Connection) -> Result<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA query_only = ON;")?;
    Ok(())
}

pub(crate) fn version(connection: &Connection) -> Result<i64> {
    Ok(connection.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

pub(crate) fn migrate(connection: &mut Connection) -> Result<i64> {
    let supported = supported_version();
    let found = version(connection)?;
    if found > supported {
        return Err(StoreError::FutureSchema { found, supported });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let target = i64::try_from(index + 1).unwrap_or(i64::MAX);
        if target <= found {
            continue;
        }
        apply(connection, target, sql).map_err(|source| StoreError::Migration {
            version: target,
            source,
        })?;
    }
    version(connection)
}

fn apply(connection: &mut Connection, target: i64, sql: &str) -> rusqlite::Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current >= target {
        return Ok(());
    }
    transaction.execute_batch(sql)?;
    transaction.execute_batch(&format!("PRAGMA user_version = {target}"))?;
    transaction.commit()?;
    tracing::info!(version = target, "store schema migrated");
    Ok(())
}
