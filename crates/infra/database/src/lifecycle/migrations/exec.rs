//! Shared SQL-execution helpers for the migration runner: transactional
//! statement application and the cross-extension `ALTER TABLE` ownership check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::DatabaseTransaction;
use crate::services::DatabaseProvider;
use std::collections::HashSet;
use std::time::Instant;
use systemprompt_extension::{Extension, LoaderError, Migration};
use systemprompt_identifiers::ToDbValue;
use tracing::{info, warn};

use super::budget;

const SLOW_STATEMENT: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) struct TrackingWrite<'a> {
    pub sql: &'a str,
    pub params: &'a [&'a dyn ToDbValue],
}

fn alter_table_targets(sql: &str) -> Result<Vec<String>, String> {
    let parsed = pg_query::parse(sql).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = Vec::new();
    for stmt in parsed.protobuf.stmts {
        let Some(node) = stmt.stmt.and_then(|s| s.node) else {
            continue;
        };
        if let pg_query::NodeEnum::AlterTableStmt(alter) = node
            && let Some(rv) = alter.relation
        {
            out.push(rv.relname);
        }
    }
    Ok(out)
}

pub(super) async fn execute_statements_transactional(
    db: &dyn DatabaseProvider,
    statements: &[String],
    ext_id: &str,
    migration: &Migration,
    tracking: Option<TrackingWrite<'_>>,
) -> Result<(), LoaderError> {
    if statements.is_empty() && tracking.is_none() {
        return Ok(());
    }

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "Failed to begin transaction for migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    let started = Instant::now();
    let total = statements.len();
    match apply_in_transaction(tx.as_mut(), statements, ext_id, migration, tracking).await {
        Ok(()) => {},
        Err(message) => {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::MigrationFailed {
                extension: ext_id.to_owned(),
                message: format!("{message}{rollback_note}"),
            });
        },
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_owned(),
            message: format!(
                "Failed to commit migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    info!(
        extension = ext_id,
        version = migration.version,
        name = migration.name,
        statements = total,
        elapsed_ms = started.elapsed().as_millis(),
        "Migration applied",
    );
    Ok(())
}

async fn apply_in_transaction(
    tx: &mut dyn DatabaseTransaction,
    statements: &[String],
    ext_id: &str,
    migration: &Migration,
    tracking: Option<TrackingWrite<'_>>,
) -> Result<(), String> {
    // Why: LOCAL, so the bound dies with this transaction and never leaks
    // onto a pooled connection the application later reuses.
    for setting in budget::timeout_statements(budget::statement_timeout(migration), true) {
        if let Err(e) = tx.execute(&setting.as_str(), &[]).await {
            return Err(format!(
                "Failed to bound migration {} ({}) with `{setting}`: {e}",
                migration.version, migration.name
            ));
        }
    }

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        let statement_started = Instant::now();
        if let Err(e) = tx.execute(&sql_str, &[]).await {
            return Err(format!(
                "Migration {ver} ({name}) statement {n}/{total} failed: {e}\nSQL:\n{statement}",
                ver = migration.version,
                name = migration.name,
                n = idx + 1,
            ));
        }
        let elapsed = statement_started.elapsed();
        if elapsed >= SLOW_STATEMENT {
            warn!(
                extension = ext_id,
                version = migration.version,
                name = migration.name,
                statement = idx + 1,
                total,
                elapsed_ms = elapsed.as_millis(),
                "Slow migration statement",
            );
        }
    }

    if let Some(write) = tracking
        && let Err(e) = tx.execute(&write.sql, write.params).await
    {
        return Err(format!(
            "Migration {ver} ({name}) tracking write failed: {e}",
            ver = migration.version,
            name = migration.name,
        ));
    }

    Ok(())
}

pub(super) fn check_cross_extension_alters(
    extension: &dyn Extension,
    migration: &Migration,
) -> Result<(), LoaderError> {
    let ext_id = extension.metadata().id;
    let altered = alter_table_targets(migration.sql).map_err(|e| LoaderError::MigrationFailed {
        extension: ext_id.to_owned(),
        message: format!(
            "Failed to parse migration {} ({}) for cross-extension ALTER check: {e}",
            migration.version, migration.name
        ),
    })?;

    if altered.is_empty() {
        return Ok(());
    }

    let mut allowed: HashSet<String> = HashSet::new();
    for schema in extension.schemas() {
        let created =
            crate::services::schema_linter::created_table_names(&schema.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!("Failed to parse declarative schema for ownership check: {e}"),
                }
            })?;
        allowed.extend(created);
    }
    for t in extension.cross_extension_tables() {
        allowed.insert(t.to_owned());
    }
    // Why: a table this extension created in an earlier migration and has
    // since dropped is still its own while the chain between those two
    // points replays on an older database.
    for earlier in extension
        .migrations()
        .iter()
        .filter(|m| !m.tombstone && m.version <= migration.version)
    {
        let created =
            crate::services::schema_linter::created_table_names(earlier.sql).map_err(|e| {
                LoaderError::MigrationFailed {
                    extension: ext_id.to_owned(),
                    message: format!(
                        "Failed to parse migration {} ({}) for ownership check: {e}",
                        earlier.version, earlier.name
                    ),
                }
            })?;
        allowed.extend(created);
    }
    for table in &altered {
        if !allowed.contains(table.as_str()) {
            return Err(LoaderError::CrossExtensionAlterUndeclared {
                extension: ext_id.to_owned(),
                table: table.clone(),
            });
        }
    }

    Ok(())
}
