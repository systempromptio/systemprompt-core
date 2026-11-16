//! [`BrainExtension`]: contributes the brain tables, views and migrations,
//! the `brain` config plane, the brain jobs and the user-purge rows of its
//! tables.
//!
//! It provides two empty late-bound handles, the [`BrainHandleSlot`] and the
//! [`BrainConsumerSlot`]. With the profile's brain enabled the kernel's
//! build of the [`BrainSlot`] assembles the brain over the built
//! [`BrainSeams`](crate::contract::BrainSeams) and binds the handle; it
//! declares no runner service, because the brain consumer needs a model
//! client the API composes, so the API starts it from the handle and binds
//! it into the consumer slot. Its backlog is reported on `/health/detail`,
//! never `/readyz`.
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
use crate::handle::BrainHandleSlot;
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
            .provide(
                |_: &Resolved<'_>| -> Result<BrainConsumerSlot, ProvideError> {
                    Ok(BrainConsumerSlot::default())
                },
            )
            .provide(
                |_: &Resolved<'_>| -> Result<BrainHandleSlot, ProvideError> {
                    Ok(BrainHandleSlot::default())
                },
            )
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
                predicate: Some(PurgePredicate {
                    column: "source_kind",
                    any_of: &["session", "entered_by", "run"],
                }),
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
