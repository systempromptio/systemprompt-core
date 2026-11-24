//! The structural and dependent DDL phases of an install, with the fresh
//! baseline stamp and the deferred foreign keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use tracing::info;

use super::foreign_keys::apply_foreign_keys;
use super::phase::execute_phase;
use super::trigger_reuse::TriggerReuse;
use super::validation::validate_extension_columns;
use super::{ExtensionSet, FreshSet};
use crate::lifecycle::installation::prepare::PreparedSchema;
use crate::lifecycle::installation::report::SchemaInstallReport;
use crate::lifecycle::migrations::MigrationService;
use crate::lifecycle::schema_name::SchemaName;
use crate::services::DatabaseProvider;

pub(super) async fn apply_structural(
    db: &dyn DatabaseProvider,
    migration_service: &MigrationService<'_>,
    schema_extensions: &ExtensionSet,
    prepared: &[PreparedSchema],
    fresh: &FreshSet,
) -> Result<(), LoaderError> {
    for (ext, p) in schema_extensions.iter().zip(prepared) {
        let stamp = if fresh.contains(&p.extension_id) {
            MigrationService::baseline_stamp_rows(ext.as_ref())
        } else {
            Vec::new()
        };
        if !stamp.is_empty() {
            info!(
                extension = %p.extension_id,
                migrations_stamped = stamp.len(),
                "Fresh install: stamping migrations as baseline without executing them"
            );
        }
        execute_phase(
            db,
            &p.structural,
            &stamp,
            &p.extension_id,
            TriggerReuse::Reapply,
        )
        .await?;
    }

    for ext in schema_extensions {
        if fresh.contains(ext.id()) {
            let retirements_run = migration_service
                .run_stamped_retirements(ext.as_ref())
                .await?;
            if retirements_run > 0 {
                info!(
                    extension = %ext.id(),
                    retirements_run,
                    "Fresh install: executed the stamped retirement migrations"
                );
            }
        }
    }
    Ok(())
}

pub(super) async fn apply_dependent(
    db: &dyn DatabaseProvider,
    schema_extensions: &ExtensionSet,
    prepared: &[PreparedSchema],
    fresh: &FreshSet,
    target: &SchemaName,
    reuse: TriggerReuse,
) -> Result<SchemaInstallReport, LoaderError> {
    for p in prepared {
        execute_phase(db, &p.dependent, &[], &p.extension_id, reuse).await?;
        for cols in &p.columns_to_validate {
            validate_extension_columns(db, cols, target, &p.extension_id).await?;
        }
    }

    // Why: after every extension's dependent phase, not inside it — a key may
    // reference a unique index another extension's dependent phase creates.
    let mut report = SchemaInstallReport::default();
    for (ext, p) in schema_extensions.iter().zip(prepared) {
        // Why: only an established extension with a migration chain can carry
        // pre-existing drift; anywhere else a key that cannot be created is a
        // schema bug and must fail the install.
        let established = ext.has_migrations() && !fresh.contains(&p.extension_id);
        let drift = apply_foreign_keys(db, &p.foreign_keys, &p.extension_id, !established).await?;
        report.foreign_key_drift.extend(drift);
    }
    Ok(report)
}
