//! One transactional DDL phase: its statements and, for a fresh install, the
//! baseline stamp rows committed alongside them so the tables and the
//! baseline claiming them can never be committed apart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use crate::lifecycle::migrations::{BaselineStamp, RECORD_MIGRATION_SQL};
use crate::services::DatabaseProvider;

pub(super) async fn execute_phase(
    db: &dyn DatabaseProvider,
    statements: &[String],
    stamp: &[BaselineStamp],
    extension_id: &ExtensionId,
) -> Result<(), LoaderError> {
    if statements.is_empty() && stamp.is_empty() {
        return Ok(());
    }

    let mut tx =
        db.begin_transaction()
            .await
            .map_err(|e| LoaderError::SchemaInstallationStepFailed {
                extension: extension_id.clone(),
                context: "Failed to begin transaction".to_owned(),
                source: Box::new(e),
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
