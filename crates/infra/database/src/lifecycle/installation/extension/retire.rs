//! The retire phase: every extension's retired triggers, functions and views
//! are dropped before any extension's migrations run.
//!
//! Extensions migrate one whole chain at a time in registry order, so when a
//! retirement is split across two of them — core drops a table, another
//! extension drops the triggers whose functions write it — the table goes
//! first and every write in between fails. A database that skips releases
//! runs both halves in one boot (0.58 → 0.60: core analytics 015 dropped
//! `analytics_ingestion_producers`, astound web 093 then fired
//! `feedback_capture` into it). Retirements run here instead, ahead of every
//! migration, so a retired object is gone before anything can fire it.
//!
//! Only `DROP … IF EXISTS` of triggers, functions, procedures and views is
//! accepted, checked before anything is written: the phase runs on every
//! boot and must be a no-op once applied. Tables are never retired here; a
//! table's data is the migration chain's to move or drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use pg_query::NodeEnum;
use pg_query::protobuf::ObjectType;
use systemprompt_extension::{Extension, LoaderError};
use tracing::info;

use super::phase::execute_phase;
use crate::services::{DatabaseProvider, SqlExecutor};

struct Retirement {
    extension: String,
    statements: Vec<String>,
}

fn refused(extension: &str, message: &str) -> LoaderError {
    LoaderError::SchemaInstallationFailed {
        extension: extension.to_owned(),
        message: format!("retirement: {message}"),
    }
}

fn check_statement(extension: &str, statement: &str) -> Result<(), LoaderError> {
    let parsed = pg_query::parse(statement)
        .map_err(|e| refused(extension, &format!("SQL parse failed: {e}")))?;
    for raw in &parsed.protobuf.stmts {
        let node = raw.stmt.as_ref().and_then(|s| s.node.as_ref());
        let Some(NodeEnum::DropStmt(drop)) = node else {
            return Err(refused(
                extension,
                &format!(
                    "only DROP … IF EXISTS is allowed, got `{}`",
                    statement.trim()
                ),
            ));
        };
        let allowed = matches!(
            ObjectType::try_from(drop.remove_type),
            Ok(ObjectType::ObjectTrigger
                | ObjectType::ObjectFunction
                | ObjectType::ObjectProcedure
                | ObjectType::ObjectView)
        );
        if !allowed {
            return Err(refused(
                extension,
                &format!(
                    "only triggers, functions, procedures and views can be retired, got `{}`",
                    statement.trim()
                ),
            ));
        }
        if !drop.missing_ok {
            return Err(refused(
                extension,
                &format!("a retirement must say IF EXISTS: `{}`", statement.trim()),
            ));
        }
    }
    Ok(())
}

fn prepare(extensions: &[Arc<dyn Extension>]) -> Result<Vec<Retirement>, LoaderError> {
    let mut out = Vec::new();
    for ext in extensions {
        let extension = ext.id().to_owned();
        let mut statements = Vec::new();
        for retirement in ext.retirements() {
            let parsed = SqlExecutor::parse_sql_statements(&retirement.sql)
                .map_err(|e| refused(&extension, &format!("SQL split failed: {e}")))?;
            for statement in parsed {
                check_statement(&extension, &statement)?;
                statements.push(statement);
            }
        }
        if !statements.is_empty() {
            out.push(Retirement {
                extension,
                statements,
            });
        }
    }
    Ok(out)
}

pub(super) fn check_retirements(extensions: &[Arc<dyn Extension>]) -> Result<(), LoaderError> {
    prepare(extensions).map(|_| ())
}

pub(super) async fn apply_retirements(
    db: &dyn DatabaseProvider,
    extensions: &[Arc<dyn Extension>],
) -> Result<(), LoaderError> {
    for retirement in prepare(extensions)? {
        execute_phase(db, &retirement.statements, &[], &retirement.extension).await?;
        info!(
            extension = %retirement.extension,
            statements = retirement.statements.len(),
            "Retirements applied"
        );
    }
    Ok(())
}
