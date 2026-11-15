//! [`BrainExtension`]: contributes the brain tables, views and migrations,
//! the `brain` config plane, the brain jobs and the user-purge rows of its
//! tables.
//!
//! It declares no runner service: the brain consumer needs the pipeline
//! seams the composition root assembles over the built
//! [`BrainSeams`](crate::contract::BrainSeams), so the root starts it and
//! binds it into the [`BrainConsumerSlot`] this member provides. Its backlog
//! is reported on `/health/detail`, never `/readyz`.
//!
//! It opens the [`BrainSlot`] itself, so the built seams are present whenever
//! this member is admitted and absent when the profile disables it.
//!
//! It requires the producer `DynNotificationSink`, which the `notifications`
//! member provides: a profile that disables notifications and keeps the
//! brain is refused when the composition resolves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_extension::{DiagnosticProbe, DynNotificationSink};

use crate::consumer::{BacklogDiagnostic, BrainConsumerSlot};
use crate::contract::BrainSlot;
use crate::jobs::{
    BrainAnalysisJob, BrainConsolidateJob, BrainRetentionJob, BrainReviewJob, BrainUsageJob,
};
use crate::plane::BrainPlane;
use crate::schema::{BRAIN_SCHEMAS, BRAIN_VIEWS};

#[derive(Debug, Clone, Copy, Default)]
pub struct BrainExtension;

impl Extension for BrainExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "brain",
            name: "Brain",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for (table, sql, columns) in BRAIN_SCHEMAS {
            c.schema(
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect()),
            );
        }
        for sql in BRAIN_VIEWS {
            c.schema(SchemaDefinition::sql_only(*sql));
        }
        c.cross_table("users")
            .migrations(extension_migrations!())
            .config_plane(Arc::new(BrainPlane))
            .requires::<DynNotificationSink>()
            .provide(|_: &Resolved<'_>| -> Result<BrainConsumerSlot, ProvideError> {
                Ok(BrainConsumerSlot::default())
            })
            .diagnostic(
                |r: &Resolved<'_>| -> Result<Arc<dyn DiagnosticProbe>, FactoryError> {
                    let slot = r.get::<BrainConsumerSlot>()?;
                    Ok(BacklogDiagnostic::new(slot).into_probe())
                },
            )
            .job(BrainUsageJob)
            .job(BrainReviewJob)
            .job(BrainRetentionJob)
            .job(BrainAnalysisJob)
            .job(BrainConsolidateJob)
            .user_purge(UserPurgeTable {
                table: "brain_facts",
                column: "owner_user_id",
                predicate: Some(
                    "source_kind = 'session' OR source_kind = 'entered_by' OR source_kind = 'run'",
                ),
            })
            .user_purge(UserPurgeTable {
                table: "brain_recalls",
                column: "principal_user_id",
                predicate: None,
            })
            .user_purge(UserPurgeTable {
                table: "brain_interests",
                column: "owner_user_id",
                predicate: None,
            });
        c.slot::<BrainSlot>();
        Ok(())
    }
}
