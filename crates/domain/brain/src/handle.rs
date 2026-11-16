//! [`BrainHandle`]: the composed brain, bound into the [`BrainHandleSlot`]
//! this member provides when the kernel builds the [`BrainSlot`].
//!
//! It holds the repositories, the producer sink, the service, recall and the
//! capture seams of the built [`BrainSeams`]. The capture consumer needs a
//! model client the API composes from the gateway, so the API server starts
//! it through [`BrainHandle::start_consumer`] and runs it as a supervised
//! runner service.
//!
//! [`BrainSlot`]: crate::BrainSlot
//! [`BrainSeams`]: crate::BrainSeams
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::brain::{BrainModelClient, DynBrainSink, MemoryRecall};
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::{AlreadyBound, LateBound};

use crate::BrainRepositories;
use crate::consumer::{BrainConsumer, BrainConsumerSlot};
use crate::pipeline::PipelineSeams;
use crate::recall::BrainRecallService;
use crate::replay::{ProposalReplay, ReplayService};
use crate::service::BrainService;

/// The late-bound [`BrainHandle`] the `brain` member provides; bound when
/// the kernel builds the brain slot with the profile's brain enabled, and
/// unbound otherwise.
pub type BrainHandleSlot = LateBound<BrainHandle>;

/// The runtime's brain: repositories, producer sink, service, recall and,
/// once the API server starts it, the capture consumer.
pub struct BrainHandle {
    repos: Arc<BrainRepositories>,
    service: Arc<BrainService>,
    recall: Arc<BrainRecallService>,
    sink: DynBrainSink,
    notifications: DynNotificationSink,
    pipeline: PipelineSeams,
    outbox: Arc<OutboxConsumer>,
    listener_pool: PgPool,
    profile: Arc<BrainProfile>,
    consumer: BrainConsumerSlot,
    consolidation_model: LateBound<dyn BrainModelClient>,
}

/// What a [`BrainHandle`] is assembled from.
pub struct BrainHandleParts {
    pub repos: Arc<BrainRepositories>,
    pub service: Arc<BrainService>,
    pub recall: Arc<BrainRecallService>,
    pub sink: DynBrainSink,
    pub notifications: DynNotificationSink,
    pub pipeline: PipelineSeams,
    pub outbox: Arc<OutboxConsumer>,
    pub listener_pool: PgPool,
    pub profile: Arc<BrainProfile>,
    pub consumer: BrainConsumerSlot,
}

impl std::fmt::Debug for BrainHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainHandle")
            .field("recall", &self.recall)
            .field("consumer", &self.consumer.is_bound())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for BrainHandleParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainHandleParts").finish_non_exhaustive()
    }
}

impl BrainHandle {
    #[must_use]
    pub fn new(parts: BrainHandleParts) -> Self {
        Self {
            repos: parts.repos,
            service: parts.service,
            recall: parts.recall,
            sink: parts.sink,
            notifications: parts.notifications,
            pipeline: parts.pipeline,
            outbox: parts.outbox,
            listener_pool: parts.listener_pool,
            profile: parts.profile,
            consumer: parts.consumer,
            consolidation_model: LateBound::default(),
        }
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<BrainRepositories> {
        &self.repos
    }

    #[must_use]
    pub const fn service(&self) -> &Arc<BrainService> {
        &self.service
    }

    #[must_use]
    pub const fn recall(&self) -> &Arc<BrainRecallService> {
        &self.recall
    }

    #[must_use]
    pub fn memory_recall(&self) -> Arc<dyn MemoryRecall> {
        let recall: Arc<dyn MemoryRecall> = Arc::<BrainRecallService>::clone(&self.recall);
        recall
    }

    #[must_use]
    pub const fn sink(&self) -> &DynBrainSink {
        &self.sink
    }

    #[must_use]
    pub const fn profile(&self) -> &Arc<BrainProfile> {
        &self.profile
    }

    #[must_use]
    pub fn routes(&self) -> (Arc<BrainService>, Arc<dyn MemoryRecall>, Arc<BrainProfile>) {
        (
            Arc::clone(&self.service),
            self.memory_recall(),
            Arc::clone(&self.profile),
        )
    }

    #[must_use]
    pub fn consumer(&self) -> Option<&Arc<BrainConsumer>> {
        self.consumer.get()
    }

    #[must_use]
    pub fn consolidation_model(&self) -> Option<Arc<dyn BrainModelClient>> {
        self.consolidation_model.get().cloned()
    }

    pub fn start_consumer(
        &self,
        model: Arc<dyn BrainModelClient>,
    ) -> Result<Arc<BrainConsumer>, AlreadyBound> {
        if self.consumer.is_bound() {
            return Err(AlreadyBound);
        }
        self.consolidation_model.bind(Arc::clone(&model))?;
        let seams = self.pipeline.clone();
        let replay = ReplayService::new(
            Arc::clone(&self.repos),
            (*self.outbox).clone(),
            seams.clone(),
            (*self.profile).clone(),
        );
        if !self.service.attach_replay(ProposalReplay {
            service: Arc::new(replay),
            model: Arc::clone(&model),
        }) {
            return Err(AlreadyBound);
        }
        let consumer = Arc::new(
            BrainConsumer::builder(
                Arc::clone(&self.repos),
                Arc::clone(&self.outbox),
                model,
                self.listener_pool.clone(),
            )
            .profile((*self.profile).clone())
            .seams(seams)
            .notifications(Arc::clone(&self.notifications))
            .build(),
        );
        self.consumer.bind(Arc::clone(&consumer))?;
        Ok(consumer)
    }
}
