//! Declarative routine preparation before migrations execute.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use tracing::{debug, warn};

use super::PreparedSchema;
use crate::services::DatabaseProvider;

// Why: migrations run before the dependent phase, so a trigger a migration
// creates would otherwise reference a function only schema/*.sql defines.
// Bodies are not checked here because a migration may add referenced columns.
// An existing function with another signature is left for migrations to
// reshape.
pub(super) async fn apply_routine_prepass(
    db: &dyn DatabaseProvider,
    prepared: &[PreparedSchema],
) -> Result<(), LoaderError> {
    for p in prepared {
        if p.routines.is_empty() {
            continue;
        }
        debug!(
            extension = %p.extension_id,
            routines = p.routines.len(),
            "Pre-applying declarative routines"
        );
        let failed = |message: String| LoaderError::SchemaInstallationFailed {
            extension: p.extension_id.clone(),
            message: format!("routine pre-pass: {message}"),
        };
        let mut tx = db
            .begin_transaction()
            .await
            .map_err(|e| failed(format!("Failed to begin transaction: {e}")))?;
        tx.execute(&"SET LOCAL check_function_bodies = off", &[])
            .await
            .map_err(|e| failed(format!("Failed to relax body checks: {e}")))?;
        for (idx, statement) in p.routines.iter().enumerate() {
            tx.execute(&"SAVEPOINT routine", &[])
                .await
                .map_err(|e| failed(format!("Failed to set savepoint: {e}")))?;
            let sql_str: &str = statement.as_str();
            match tx.execute(&sql_str, &[]).await {
                Ok(_) => {},
                Err(e) if e.is_invalid_function_definition() => {
                    warn!(
                        extension = %p.extension_id,
                        error = %e,
                        "Declarative routine already exists with another signature; \
                         leaving it for the migrations to reshape"
                    );
                    tx.execute(&"ROLLBACK TO SAVEPOINT routine", &[])
                        .await
                        .map_err(|e| failed(format!("Failed to roll back savepoint: {e}")))?;
                },
                Err(e) => {
                    let rollback_note = match tx.rollback().await {
                        Ok(()) => String::new(),
                        Err(rb) => format!(" (rollback also failed: {rb})"),
                    };
                    return Err(failed(format!(
                        "Statement {n}/{total} failed: {e}{rollback_note}\nSQL:\n{statement}",
                        n = idx + 1,
                        total = p.routines.len(),
                    )));
                },
            }
        }
        tx.commit()
            .await
            .map_err(|e| failed(format!("Failed to commit transaction: {e}")))?;
    }
    Ok(())
}
