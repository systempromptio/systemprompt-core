//! [`BrainExtension`]: contributes the brain tables, views and migrations,
//! the `brain` config plane, the brain jobs and the user-purge rows of its
//! tables.
//!
//! It opens the [`BrainSlot`] itself, so the slot's built value, the
//! [`BrainHandleSlot`], is present whenever this member is admitted. The build
//! creates the handle cell and, with the profile's brain enabled, binds the
//! brain it assembles over the built
//! [`BrainSeams`](crate::contract::BrainSeams); no member provides or binds
//! the handle. The member declares no runner service, because the brain
//! consumer needs a model client the API composes, so the API starts it from
//! the handle through the `DynBrainConsumerStart` it provides **sealed**
//! (only kernel code starts the consumer). Its backlog is reported on
//! `/health/detail`, never `/readyz`.
//!
//! It provides the brain's outbox sink (`DynBrainSink`, open), which the
//! document store and the gateway's session capture read only while the
//! profile enables the brain, and, after the slot is built, the brain's
//! [`MemoryRecall`] (open): it
//! recalls through the bound handle and answers `skipped=disabled` while the
//! handle is unbound. Recall reads facts as the principal it is handed.
//!
//! It requires the producer `DynNotificationSink`, which the `notifications`
//! member provides: a profile that disables notifications and keeps the
//! brain is refused when the composition resolves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::brain::{
    BrainConsumerStart, BrainModelClient, DynBrainConsumerStart, DynBrainSink, MemoryRecall,
};
use systemprompt_extension::prelude::*;
use systemprompt_extension::{DiagnosticProbe, DynNotificationSink};
use systemprompt_identifiers::InstanceId;
use systemprompt_models::AlreadyBound;
use systemprompt_traits::RunnerService;

use crate::OutboxBrainSink;
use crate::consumer::BacklogDiagnostic;
use crate::contract::BrainSlot;
use crate::handle::BrainHandleSlot;
use crate::jobs::{
    BrainAnalysisJob, BrainConsolidateJob, BrainRetentionJob, BrainReviewJob, BrainUsageJob,
};
use crate::plane::BrainPlane;
use crate::recall::BoundMemoryRecall;
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
            .provide(|r: &Resolved<'_>| -> Result<DynBrainSink, ProvideError> {
                let db = r.get::<DbPool>()?;
                let instance = r.get::<InstanceId>()?;
                Ok(OutboxBrainSink::over_write_pool(&db, &instance))
            })
            .provide_after_governance(
                |r: &Resolved<'_>| -> Result<Arc<dyn MemoryRecall>, ProvideError> {
                    let brain = r.get::<Arc<BrainHandleSlot>>()?;
                    Ok(Arc::new(BoundMemoryRecall::new(brain)))
                },
            )
            .provide_sealed_after_governance(
                |r: &Resolved<'_>| -> Result<DynBrainConsumerStart, ProvideError> {
                    Ok(Arc::new(HandleConsumerStart(
                        r.get::<Arc<BrainHandleSlot>>()?,
                    )))
                },
            )
            .diagnostic(
                |r: &Resolved<'_>| -> Result<Arc<dyn DiagnosticProbe>, FactoryError> {
                    let brain = r.get::<Arc<BrainHandleSlot>>()?;
                    Ok(BacklogDiagnostic::new(brain).into_probe())
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

struct HandleConsumerStart(Arc<BrainHandleSlot>);

impl BrainConsumerStart for HandleConsumerStart {
    fn start(
        &self,
        model: Arc<dyn BrainModelClient>,
    ) -> Result<Option<Arc<dyn RunnerService>>, AlreadyBound> {
        let Some(handle) = self.0.get() else {
            return Ok(None);
        };
        let consumer: Arc<dyn RunnerService> = handle.start_consumer(model)?;
        Ok(Some(consumer))
    }
}
