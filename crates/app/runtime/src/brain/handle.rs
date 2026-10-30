//! [`BrainHandle`]: the composed brain the context hands to the API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use sqlx::PgPool;
use systemprompt_brain::BrainRepositories;
use systemprompt_brain::consumer::BrainConsumer;
use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_brain::recall::BrainRecallService;
use systemprompt_brain::service::BrainService;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::ExtensionContext;
use systemprompt_extension::brain::{BrainModelClient, DynBrainSink, MemoryRecall, SourceContent};
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_manifest::profile::BrainProfile;

/// The runtime's brain: repositories, producer sink, service, recall and,
/// once the API server starts it, the capture consumer.
pub struct BrainHandle {
    repos: Arc<BrainRepositories>,
    service: Arc<BrainService>,
    recall: Arc<BrainRecallService>,
    sink: DynBrainSink,
    notifications: DynNotificationSink,
    source_content: Arc<dyn SourceContent>,
    outbox: Arc<OutboxConsumer>,
    listener_pool: PgPool,
    profile: Arc<BrainProfile>,
    consumer: OnceLock<Arc<BrainConsumer>>,
}

/// What a [`BrainHandle`] is assembled from.
pub struct BrainHandleParts {
    pub repos: Arc<BrainRepositories>,
    pub service: Arc<BrainService>,
    pub recall: Arc<BrainRecallService>,
    pub sink: DynBrainSink,
    pub notifications: DynNotificationSink,
    pub source_content: Arc<dyn SourceContent>,
    pub outbox: Arc<OutboxConsumer>,
    pub listener_pool: PgPool,
    pub profile: Arc<BrainProfile>,
}

impl std::fmt::Debug for BrainHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainHandle")
            .field("recall", &self.recall)
            .field("consumer", &self.consumer.get().is_some())
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
            source_content: parts.source_content,
            outbox: parts.outbox,
            listener_pool: parts.listener_pool,
            profile: parts.profile,
            consumer: OnceLock::new(),
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

    pub fn mcp_tools(&self) -> Result<super::mcp::BrainMcpTools, super::mcp::BrainMcpUnavailable> {
        super::mcp::BrainMcpTools::from_parts(
            Arc::clone(&self.service),
            self.memory_recall(),
            Arc::clone(&self.profile),
        )
    }

    #[must_use]
    pub fn consumer(&self) -> Option<&Arc<BrainConsumer>> {
        self.consumer.get()
    }

    pub fn start_consumer(
        &self,
        extensions: &dyn ExtensionContext,
        model: Arc<dyn BrainModelClient>,
    ) -> Arc<BrainConsumer> {
        let consumer = self.consumer.get_or_init(|| {
            let mut seams = PipelineSeams::discover(extensions);
            seams.sources.push(Arc::clone(&self.source_content));
            Arc::new(
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
            )
        });
        Arc::clone(consumer)
    }
}
