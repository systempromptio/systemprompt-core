//! Shared SQL-execution helpers for the migration runner: transactional
//! statement application and the cross-extension `ALTER TABLE` ownership check.

use crate::services::DatabaseProvider;
use std::collections::HashSet;
use systemprompt_extension::{Extension, LoaderError, Migration};

/// Inspect a migration's SQL with `pg_query` and return every table name that
/// appears as the target of an `ALTER TABLE` statement. Parser errors are
/// returned as `Err` so callers can attribute them to the originating
/// extension / migration.
fn alter_table_targets(sql: &str) -> Result<Vec<String>, String> {
    let parsed = pg_query::parse(sql).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = Vec::new();
    for stmt in parsed.protobuf.stmts {
        let Some(node) = stmt.stmt.and_then(|s| s.node) else {
            continue;
        };
        if let pg_query::NodeEnum::AlterTableStmt(alter) = node {
            if let Some(rv) = alter.relation {
                out.push(rv.relname);
            }
        }
    }
    Ok(out)
}

/// Apply an already-parsed statement list inside a `BEGIN; … COMMIT` envelope,
/// falling back to `ROLLBACK` on any failure. The bookkeeping write must only
/// be recorded after the commit succeeds.
pub(super) async fn execute_statements_transactional(
    db: &dyn DatabaseProvider,
    statements: &[String],
    ext_id: &str,
    migration: &Migration,
) -> Result<(), LoaderError> {
    if statements.is_empty() {
        return Ok(());
    }

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_string(),
            message: format!(
                "Failed to begin transaction for migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        if let Err(e) = tx.execute(&sql_str, &[]).await {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::MigrationFailed {
                extension: ext_id.to_string(),
                message: format!(
                    "Migration {ver} ({name}) statement {n}/{total} failed: \
                     {e}{rollback_note}\nSQL:\n{statement}",
                    ver = migration.version,
                    name = migration.name,
                    n = idx + 1,
                ),
            });
        }
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::MigrationFailed {
            extension: ext_id.to_string(),
            message: format!(
                "Failed to commit migration {} ({}): {e}",
                migration.version, migration.name
            ),
        })?;

    Ok(())
}

/// Reject a migration whose `ALTER TABLE` targets a table the extension has not
/// declared in `owned_tables()` or `cross_extension_tables()`. Cross-extension
/// table mutations must be declared explicitly.
pub(super) fn check_cross_extension_alters(
    extension: &dyn Extension,
    migration: &Migration,
) -> Result<(), LoaderError> {
    let ext_id = extension.metadata().id;
    let altered = alter_table_targets(migration.sql).map_err(|e| LoaderError::MigrationFailed {
        extension: ext_id.to_string(),
        message: format!(
            "Failed to parse migration {} ({}) for cross-extension ALTER check: {e}",
            migration.version, migration.name
        ),
    })?;

    if altered.is_empty() {
        return Ok(());
    }

    let mut allowed: HashSet<&str> = HashSet::new();
    for t in extension.owned_tables() {
        allowed.insert(t);
    }
    for t in extension.cross_extension_tables() {
        allowed.insert(t);
    }
    for table in &altered {
        if !allowed.contains(table.as_str()) {
            return Err(LoaderError::CrossExtensionAlterUndeclared {
                extension: ext_id.to_string(),
                table: table.clone(),
            });
        }
    }

    Ok(())
}
