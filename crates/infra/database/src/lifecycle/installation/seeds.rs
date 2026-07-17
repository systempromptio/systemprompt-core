//! Idempotent seed application.
//!
//! Seeds run after an extension's schemas and migrations, on every boot, and
//! are deliberately not tracked in `extension_migrations` — every seed body
//! must be idempotent. The classifier rejects any statement that is not
//! `INSERT`, `UPDATE`, or `MERGE`.

use crate::services::DatabaseProvider;
use systemprompt_extension::{Extension, LoaderError, Seed};
use tracing::{debug, info};

pub(super) async fn apply_seeds(
    extension: &dyn Extension,
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    let seeds = extension.seeds();
    if seeds.is_empty() {
        return Ok(());
    }

    let ext_id = extension.metadata().id;

    let mut statement_lists = Vec::with_capacity(seeds.len());
    for seed in &seeds {
        statement_lists.push(lint_seed(ext_id, seed)?);
    }

    info!(extension = %ext_id, count = seeds.len(), "Applying seeds");

    for (seed, statements) in seeds.iter().zip(&statement_lists) {
        apply_one(ext_id, seed, statements, db).await?;
    }

    Ok(())
}

async fn apply_one(
    ext_id: &str,
    seed: &Seed,
    statements: &[String],
    db: &dyn DatabaseProvider,
) -> Result<(), LoaderError> {
    debug!(extension = %ext_id, seed = %seed.id, "Applying seed");

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| LoaderError::SeedFailed {
            extension: ext_id.to_owned(),
            seed: seed.id.to_owned(),
            message: format!("begin transaction: {e}"),
        })?;

    // Why: one prepared execute per statement — Postgres rejects multi-command
    // prepared statements, and multi-statement seed bodies are valid input.
    for statement in statements {
        if let Err(e) = tx.execute(statement, &[]).await {
            let rollback = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::SeedFailed {
                extension: ext_id.to_owned(),
                seed: seed.id.to_owned(),
                message: format!("execute: {e}{rollback}"),
            });
        }
    }

    tx.commit().await.map_err(|e| LoaderError::SeedFailed {
        extension: ext_id.to_owned(),
        seed: seed.id.to_owned(),
        message: format!("commit: {e}"),
    })?;

    Ok(())
}

fn lint_seed(ext_id: &str, seed: &Seed) -> Result<Vec<String>, LoaderError> {
    let parsed = pg_query::parse(seed.sql).map_err(|e| LoaderError::SeedFailed {
        extension: ext_id.to_owned(),
        seed: seed.id.to_owned(),
        message: format!("parse: {e}"),
    })?;

    let mut statements = Vec::with_capacity(parsed.protobuf.stmts.len());
    for stmt in &parsed.protobuf.stmts {
        let Some(node) = stmt.stmt.as_ref().and_then(|s| s.node.as_ref()) else {
            continue;
        };
        statements.push(statement_text(seed.sql, stmt));
        let kind = classify(node);
        if !is_allowed(kind) {
            return Err(LoaderError::InvalidSeedStatement {
                extension: ext_id.to_owned(),
                seed: seed.id.to_owned(),
                statement: kind.to_owned(),
            });
        }
        if let pg_query::NodeEnum::InsertStmt(insert) = node
            && insert.on_conflict_clause.is_none()
        {
            return Err(LoaderError::SeedInsertNotIdempotent {
                extension: ext_id.to_owned(),
                seed: seed.id.to_owned(),
            });
        }
    }

    Ok(statements)
}

fn statement_text(sql: &str, stmt: &pg_query::protobuf::RawStmt) -> String {
    let start = usize::try_from(stmt.stmt_location).unwrap_or(0);
    let end = if stmt.stmt_len > 0 {
        start.saturating_add(usize::try_from(stmt.stmt_len).unwrap_or(0))
    } else {
        sql.len()
    };
    sql.get(start..end).unwrap_or(sql).trim().to_owned()
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
