//! Persisted user-purge declarations: what each member's user purge clears.
//!
//! A disabled member's code never runs, so its `user_purge`/`orphan_sweep`
//! declarations cannot be read from the composition while it is disabled.
//! Each install therefore admits every member's declarations (the same
//! ownership check the kernel runs, [`AdmittedPurge::admit`]), validates
//! every name they carry as an identifier, and records them per member in
//! `schema_purge_records` with the member's composition position. The user
//! purge runs the recorded declarations of every member absent from its
//! composition. A member's row is replaced whole, in one statement, at each
//! install in which it is admitted; a member with no declarations records an
//! empty set, so a missing row means the member was never admitted since the
//! records existed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{AdmittedPurge, LoaderError, PurgeClaim, PurgeDeclaration, SchemaDefinition};
use systemprompt_identifiers::ExtensionId;

use super::ExtensionSet;
use crate::admin::SafeIdentifier;
use crate::services::DatabaseProvider;

const PRESENT_SQL: &str =
    "SELECT to_regclass('schema_purge_records') IS NOT NULL AS present";

const RECORD_SQL: &str = "INSERT INTO schema_purge_records (extension_id, position, declarations) \
     VALUES ($1, $2, $3::jsonb) \
     ON CONFLICT (extension_id) DO UPDATE SET position = EXCLUDED.position, \
     declarations = EXCLUDED.declarations, recorded_at = CURRENT_TIMESTAMP";

pub(super) fn admit_purges(members: &ExtensionSet) -> Result<(), LoaderError> {
    let ids: Vec<ExtensionId> = members.iter().map(|m| ExtensionId::new(m.id())).collect();
    let schemas: Vec<Vec<SchemaDefinition>> = members.iter().map(|m| m.schemas()).collect();
    let purges: Vec<Vec<PurgeDeclaration>> = members.iter().map(|m| m.purges()).collect();
    let claims: Vec<PurgeClaim<'_>> = members
        .iter()
        .enumerate()
        .map(|(i, m)| PurgeClaim {
            owner: &ids[i],
            standing: m.standing(),
            schemas: &schemas[i],
            declarations: &purges[i],
        })
        .collect();
    AdmittedPurge::admit(&claims)?;
    for (owner, declarations) in ids.iter().zip(&purges) {
        for name in declarations.iter().flat_map(PurgeDeclaration::identifiers) {
            SafeIdentifier::parse(name).map_err(|source| {
                LoaderError::SchemaInstallationStepFailed {
                    extension: owner.clone(),
                    context: format!("user purge declaration names '{name}', not an identifier"),
                    source: Box::new(source),
                }
            })?;
        }
    }
    Ok(())
}

pub(super) async fn record_purges(
    db: &dyn DatabaseProvider,
    members: &ExtensionSet,
) -> Result<(), LoaderError> {
    let present = db
        .fetch_one(&PRESENT_SQL, &[])
        .await
        .map_err(|e| step_failed("Failed to look up the user purge records", Box::new(e)))?
        .get("present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !present {
        return Ok(());
    }
    for (position, member) in members.iter().enumerate() {
        let declarations = serde_json::to_string(&member.purges())
            .map_err(|e| step_failed("Failed to encode user purge declarations", Box::new(e)))?;
        let position = i32::try_from(position).unwrap_or(i32::MAX);
        db.execute(&RECORD_SQL, &[&member.id(), &position, &declarations])
            .await
            .map_err(|e| step_failed("Failed to record user purge declarations", Box::new(e)))?;
    }
    Ok(())
}

fn step_failed(context: &str, source: systemprompt_traits::BoxedSource) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: context.to_owned(),
        source,
    }
}
