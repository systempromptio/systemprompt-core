//! The [`BrainHandle`] assembled over the built [`BrainSeams`].
//!
//! When the profile enables the brain, the slot build composes the brain
//! repositories, the outbox sink, the [`BrainService`] and the
//! [`BrainRecallService`] from the kernel's database pool and instance id,
//! the producer notification sink and the built seams, and binds the handle
//! into the [`BrainHandleSlot`] this member provides. With the brain
//! disabled nothing is composed and the slot stays unbound. Recall and the
//! service judge a fact's source by the built seams' source access; without
//! one every sourced fact is hidden.
//!
//! [`BrainService`]: crate::service::BrainService
//! [`BrainRecallService`]: crate::recall::BrainRecallService
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::brain::{FactRecallSource, SourceAccess};
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_extension::{Resolved, SlotError};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::BrainProfile;

use crate::consumer::BrainConsumerSlot;
use crate::contract::BrainSeams;
use crate::handle::{BrainHandle, BrainHandleParts, BrainHandleSlot};
use crate::outcomes::OutcomeService;
use crate::recall::{BrainRecallService, BrainRecallStore, DeclaredSource, PlaneDeclaredSource};
use crate::service::{BrainService, NoSourceAccess};
use crate::{BrainRepositories, OutboxBrainSink};

const SLOT: &str = "brain";

pub(crate) fn bind_handle(seams: &BrainSeams, r: &Resolved<'_>) -> Result<(), SlotError> {
    let profile = r.get::<Arc<BrainProfile>>()?;
    if !profile.enabled {
        return Ok(());
    }
    let db = r.get::<DbPool>()?;
    let instance = r.get::<InstanceId>()?;
    let notifications = r.get::<DynNotificationSink>()?;
    let repos = BrainRepositories::new(&db);
    let sink = OutboxBrainSink::over_write_pool(&db, &instance);
    let declared: Arc<dyn DeclaredSource> = Arc::new(PlaneDeclaredSource::new(repos.clone()));
    let source_access: Arc<dyn SourceAccess> = seams
        .source_access
        .clone()
        .unwrap_or_else(|| Arc::new(NoSourceAccess));
    let service = BrainService::builder(repos.clone(), Arc::clone(&sink), Arc::clone(&profile))
        .notifications(Arc::clone(&notifications))
        .declared(Arc::clone(&declared))
        .source_access(Arc::clone(&source_access))
        .audiences(seams.audiences.clone())
        .build();
    let store = Arc::new(BrainRecallStore::new(repos.clone(), profile.recall.stale));
    let source: Arc<dyn FactRecallSource> = Arc::<BrainRecallStore>::clone(&store);
    let recall =
        BrainRecallService::builder(source, store, source_access, declared, Arc::clone(&profile))
            .retrievers(seams.retrievers.clone())
            .rerankers(seams.rerankers.clone())
            .injection_policies(seams.injection.clone())
            .audiences(seams.audiences.clone())
            .importance(seams.importance.clone())
            .outcomes(OutcomeService::new(
                Arc::clone(&repos.recalls),
                Arc::clone(&repos.outcomes),
                seams.outcome_signals.clone(),
            ))
            .build();
    let handle = BrainHandle::new(BrainHandleParts {
        repos: Arc::new(repos),
        service: Arc::new(service),
        recall: Arc::new(recall),
        sink,
        notifications,
        pipeline: seams.pipeline(),
        outbox: Arc::new(OutboxConsumer::new(db.write_pool().as_ref().clone())),
        listener_pool: db.session_pool().as_ref().clone(),
        profile,
        consumer: r.get::<BrainConsumerSlot>()?,
    });
    r.get::<BrainHandleSlot>()?
        .bind(Arc::new(handle))
        .map_err(|error| SlotError::Refused {
            slot: SLOT,
            source: Box::new(error),
        })
}
