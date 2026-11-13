//! One transactional DDL phase: its statements and, for a fresh install, the
//! baseline stamp rows committed alongside them so the tables and the
//! baseline claiming them can never be committed apart.
//!
//! The phase re-runs every boot, and re-creating a trigger takes an exclusive
//! lock on its table. On a rolling deploy the other nodes are serving, so a
//! long reader there would queue this boot, and every writer behind it,
//! indefinitely. The phase runs under the migration `lock_timeout` instead,
//! and a lock conflict rolls it back and retries with jittered backoff: the
//! reader finishes, the boot proceeds, and no writer waits longer than the
//! bound. A lock conflict is a lock timeout, a deadlock with live writers or
//! a serialization failure, judged by SQLSTATE
//! ([`super::super::lock_conflict`]); the jitter keeps two installers that
//! deadlocked each other from retrying in lockstep.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use super::super::lock_conflict::is_retryable_lock_conflict;
use crate::lifecycle::migrations::{BaselineStamp, RECORD_MIGRATION_SQL, budget};
use crate::services::DatabaseProvider;

const LOCK_ATTEMPTS: u32 = 6;

pub(super) async fn execute_phase(
    db: &dyn DatabaseProvider,
    statements: &[String],
    stamp: &[BaselineStamp],
    extension_id: &ExtensionId,
) -> Result<(), LoaderError> {
    if statements.is_empty() && stamp.is_empty() {
        return Ok(());
    }
    let mut attempt = 1;
    loop {
        match attempt_phase(db, statements, stamp, extension_id).await {
            Err(error) if attempt < LOCK_ATTEMPTS && is_retryable_lock_conflict(&error) => {
                tracing::warn!(
                    extension = %extension_id,
                    attempt,
                    error = %error,
                    cause = ?std::error::Error::source(&error),
                    "Schema phase hit a lock conflict on a busy table; retrying",
                );
                tokio::time::sleep(retry_delay(attempt)).await;
                attempt += 1;
            },
            outcome => return outcome,
        }
    }
}

fn retry_delay(attempt: u32) -> std::time::Duration {
    let base = u64::from(attempt) * 2000;
    let jitter = rand::random_range(0..=base / 2);
    std::time::Duration::from_millis(base + jitter)
}

async fn attempt_phase(
    db: &dyn DatabaseProvider,
    statements: &[String],
    stamp: &[BaselineStamp],
    extension_id: &ExtensionId,
) -> Result<(), LoaderError> {
    let mut tx =
        db.begin_transaction()
            .await
            .map_err(|e| LoaderError::SchemaInstallationStepFailed {
                extension: extension_id.clone(),
                context: "Failed to begin transaction".to_owned(),
                source: Box::new(e),
            })?;

    let bound = format!(
        "SET LOCAL lock_timeout = {}",
        budget::LOCK_TIMEOUT.as_millis()
    );
    tx.execute(&bound.as_str(), &[]).await.map_err(|e| {
        LoaderError::SchemaInstallationStepFailed {
            extension: extension_id.clone(),
            context: "Failed to bound the phase's lock wait".to_owned(),
            source: Box::new(e),
        }
    })?;

    let total = statements.len();
    for (idx, statement) in statements.iter().enumerate() {
        let sql_str: &str = statement.as_str();
        if let Err(e) = tx.execute(&sql_str, &[]).await {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::SchemaInstallationStepFailed {
                extension: extension_id.clone(),
                context: format!(
                    "Statement {n}/{total} failed{rollback_note}\nSQL:\n{statement}",
                    n = idx + 1,
                ),
                source: Box::new(e),
            });
        }
    }

    for row in stamp {
        let params: [&dyn systemprompt_identifiers::ToDbValue; 5] = [
            &row.id,
            &extension_id,
            &row.version,
            &row.name,
            &row.checksum,
        ];
        if let Err(e) = tx.execute(&RECORD_MIGRATION_SQL, &params).await {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(LoaderError::SchemaInstallationStepFailed {
                extension: extension_id.clone(),
                context: format!(
                    "Failed to stamp migration {} ({}) as applied{rollback_note}",
                    row.version, row.name
                ),
                source: Box::new(e),
            });
        }
    }

    tx.commit()
        .await
        .map_err(|e| LoaderError::SchemaInstallationStepFailed {
            extension: extension_id.clone(),
            context: "Failed to commit transaction".to_owned(),
            source: Box::new(e),
        })?;

    Ok(())
}
