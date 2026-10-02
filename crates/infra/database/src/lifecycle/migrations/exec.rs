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
use systemprompt_identifiers::{ExtensionId, ToDbValue};
use tracing::{info, warn};

use super::step_error::MigrationStepError;
use super::{budget, triggers};

const SLOW_STATEMENT: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) struct TrackingWrite<'a> {
    pub sql: &'a str,
    pub params: &'a [&'a dyn ToDbValue],
}

fn alter_table_targets(sql: &str) -> Result<Vec<String>, pg_query::Error> {
    let parsed = pg_query::parse(sql)?;
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
    ext_id: &ExtensionId,
    migration: &Migration,
    tracking: Option<TrackingWrite<'_>>,
) -> Result<(), LoaderError> {
    if statements.is_empty() && tracking.is_none() {
        return Ok(());
    }

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::MigrationStepFailed {
            extension: ext_id.clone(),
            context: format!(
                "Failed to begin transaction for migration {} ({})",
                migration.version, migration.name
            ),
            source: Box::new(e),
        })?;

    let started = Instant::now();
    let total = statements.len();
    match apply_in_transaction(tx.as_mut(), statements, ext_id, migration, tracking).await {
        Ok(()) => {},
        Err(step) => {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::MigrationStepFailed {
                extension: ext_id.clone(),
                context: format!("transaction rolled back{rollback_note}"),
                source: Box::new(step),
            });
        },
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::MigrationStepFailed {
            extension: ext_id.clone(),
            context: format!(
                "Failed to commit migration {} ({})",
                migration.version, migration.name
            ),
            source: Box::new(e),
        })?;

    info!(
        extension = %ext_id,
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
    ext_id: &ExtensionId,
    migration: &Migration,
    tracking: Option<TrackingWrite<'_>>,
) -> Result<(), MigrationStepError> {
    // Why: LOCAL, so the bound dies with this transaction and never leaks
    // onto a pooled connection the application later reuses.
    for setting in budget::timeout_statements(budget::statement_timeout(migration), true) {
        if let Err(source) = tx.execute(&setting.as_str(), &[]).await {
            return Err(MigrationStepError::Bound {
                version: migration.version,
                name: migration.name.clone(),
                setting,
                source,
            });
        }
    }

    let suspended = triggers::suspend(&mut triggers::Target::Tx(&mut *tx), migration).await?;
    if !suspended.is_empty() {
        info!(
            extension = %ext_id,
            version = migration.version,
            name = migration.name,
            triggers = %suspended.describe(),
            "Row triggers suspended for migration",
        );
    }

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        let statement_started = Instant::now();
        if let Err(source) = tx.execute(&sql_str, &[]).await {
            return Err(MigrationStepError::Statement {
                version: migration.version,
                name: migration.name.clone(),
                n: idx + 1,
                total,
                statement: statement.clone(),
                source,
            });
        }
        let elapsed = statement_started.elapsed();
        if elapsed >= SLOW_STATEMENT {
            warn!(
                extension = %ext_id,
                version = migration.version,
                name = migration.name,
                statement = idx + 1,
                total,
                elapsed_ms = elapsed.as_millis(),
                "Slow migration statement",
            );
        }
    }

    suspended
        .restore(&mut triggers::Target::Tx(&mut *tx))
        .await?;

    if let Some(write) = tracking
        && let Err(source) = tx.execute(&write.sql, write.params).await
    {
        return Err(MigrationStepError::Tracking {
            version: migration.version,
            name: migration.name.clone(),
            source,
        });
    }

    Ok(())
}

pub(super) fn check_cross_extension_alters(
    extension: &dyn Extension,
    migration: &Migration,
) -> Result<(), LoaderError> {
    let ext_id = &ExtensionId::new(extension.metadata().id);
    let altered =
        alter_table_targets(migration.sql).map_err(|e| LoaderError::MigrationStepFailed {
            extension: ext_id.clone(),
            context: format!(
                "Failed to parse migration {} ({}) for cross-extension ALTER check",
                migration.version, migration.name
            ),
            source: Box::new(e),
        })?;

    if altered.is_empty() {
        return Ok(());
    }

    let mut allowed: HashSet<String> = HashSet::new();
    for schema in extension.schemas() {
        let created =
            crate::services::schema_linter::created_table_names(&schema.sql).map_err(|e| {
                LoaderError::MigrationStepFailed {
                    extension: ext_id.clone(),
                    context: "Failed to parse declarative schema for ownership check".to_owned(),
                    source: Box::new(e),
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
                LoaderError::MigrationStepFailed {
                    extension: ext_id.clone(),
                    context: format!(
                        "Failed to parse migration {} ({}) for ownership check",
                        earlier.version, earlier.name
                    ),
                    source: Box::new(e),
                }
            })?;
        allowed.extend(created);
    }
    for table in &altered {
        if !allowed.contains(table.as_str()) {
            return Err(LoaderError::CrossExtensionAlterUndeclared {
                extension: ext_id.clone(),
                table: table.clone(),
            });
        }
    }

    Ok(())
}
