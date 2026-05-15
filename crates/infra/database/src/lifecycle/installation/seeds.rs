//! Idempotent seed application.
//!
//! Seeds run after an extension's schemas and migrations, on every boot, and
//! are deliberately not tracked in `extension_migrations` — every seed body
//! must be idempotent. The classifier rejects any statement that is not
//! `INSERT`, `UPDATE`, or `MERGE`.

use crate::services::DatabaseProvider;
use systemprompt_extension::{Extension, LoaderError, Seed};
use tracing::{debug, info};

pub async fn apply_seeds(
    extension: &dyn Extension,
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    let seeds = extension.seeds();
    if seeds.is_empty() {
        return Ok(());
    }

    let ext_id = extension.metadata().id;

    for seed in &seeds {
        lint_seed(ext_id, seed)?;
    }

    info!(extension = %ext_id, count = seeds.len(), "Applying seeds");

    for seed in &seeds {
        apply_one(ext_id, seed, db).await?;
    }

    Ok(())
}

async fn apply_one(
    ext_id: &str,
    seed: &Seed,
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    debug!(extension = %ext_id, seed = %seed.id, "Applying seed");

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::SeedFailed {
            extension: ext_id.to_string(),
            seed: seed.id.to_string(),
            message: format!("begin transaction: {e}"),
        })?;

    if let Err(e) = tx.execute(&seed.sql, &[]).await {
        let rollback = match tx.rollback().await {
            Ok(()) => String::new(),
            Err(rb) => format!(" (rollback also failed: {rb})"),
        };
        return Err(LoaderError::SeedFailed {
            extension: ext_id.to_string(),
            seed: seed.id.to_string(),
            message: format!("execute: {e}{rollback}"),
        });
    }

    tx.commit().await.map_err(|e| LoaderError::SeedFailed {
        extension: ext_id.to_string(),
        seed: seed.id.to_string(),
        message: format!("commit: {e}"),
    })?;

    Ok(())
}

fn lint_seed(ext_id: &str, seed: &Seed) -> Result<(), LoaderError> {
    let parsed = pg_query::parse(seed.sql).map_err(|e| LoaderError::SeedFailed {
        extension: ext_id.to_string(),
        seed: seed.id.to_string(),
        message: format!("parse: {e}"),
    })?;

    for stmt in &parsed.protobuf.stmts {
        let Some(node) = stmt.stmt.as_ref().and_then(|s| s.node.as_ref()) else {
            continue;
        };
        let kind = classify(node);
        if !is_allowed(kind) {
            return Err(LoaderError::InvalidSeedStatement {
                extension: ext_id.to_string(),
                seed: seed.id.to_string(),
                statement: kind.to_string(),
            });
        }
    }

    Ok(())
}

const fn classify(node: &pg_query::NodeEnum) -> &'static str {
    match node {
        pg_query::NodeEnum::InsertStmt(_) => "INSERT",
        pg_query::NodeEnum::UpdateStmt(_) => "UPDATE",
        pg_query::NodeEnum::MergeStmt(_) => "MERGE",
        pg_query::NodeEnum::SelectStmt(_) => "SELECT",
        pg_query::NodeEnum::DeleteStmt(_) => "DELETE",
        pg_query::NodeEnum::CreateStmt(_)
        | pg_query::NodeEnum::CreateSchemaStmt(_)
        | pg_query::NodeEnum::CreateTableAsStmt(_)
        | pg_query::NodeEnum::CreateExtensionStmt(_)
        | pg_query::NodeEnum::CreateFunctionStmt(_)
        | pg_query::NodeEnum::CreateTrigStmt(_)
        | pg_query::NodeEnum::ViewStmt(_)
        | pg_query::NodeEnum::IndexStmt(_) => "CREATE",
        pg_query::NodeEnum::AlterTableStmt(_)
        | pg_query::NodeEnum::AlterDatabaseStmt(_)
        | pg_query::NodeEnum::AlterFunctionStmt(_)
        | pg_query::NodeEnum::AlterRoleStmt(_) => "ALTER",
        pg_query::NodeEnum::DropStmt(_) | pg_query::NodeEnum::DropRoleStmt(_) => "DROP",
        pg_query::NodeEnum::TruncateStmt(_) => "TRUNCATE",
        pg_query::NodeEnum::GrantStmt(_) => "GRANT",
        _ => "OTHER",
    }
}

fn is_allowed(kind: &str) -> bool {
    matches!(kind, "INSERT" | "UPDATE" | "MERGE")
}
