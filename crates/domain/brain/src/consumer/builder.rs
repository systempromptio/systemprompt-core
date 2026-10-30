//! [`BrainConsumerBuilder`]: assembles the consumer and its pipeline from
//! the collaborators the composition root injects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::brain::BrainModelClient;
use systemprompt_extension::notifications::DynNotificationSink;
use systemprompt_manifest::profile::BrainProfile;

use super::{BrainConsumer, Counters, StateCell};
use crate::pipeline::{BrainPipeline, PipelineSeams};
use crate::repository::BrainRepositories;

/// Builds a [`BrainConsumer`]; the profile defaults to the brain
/// defaults, the seams to none and notifications to unavailable.
pub struct BrainConsumerBuilder {
    repos: Arc<BrainRepositories>,
    outbox: Arc<OutboxConsumer>,
    model: Arc<dyn BrainModelClient>,
    listener_pool: PgPool,
    profile: BrainProfile,
    seams: PipelineSeams,
    notifications: Option<DynNotificationSink>,
}

impl std::fmt::Debug for BrainConsumerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainConsumerBuilder")
            .field("profile", &self.profile)
            .field("seams", &self.seams)
            .finish_non_exhaustive()
    }
}

impl BrainConsumer {
    #[must_use]
    pub fn builder(
        repos: Arc<BrainRepositories>,
        outbox: Arc<OutboxConsumer>,
        model: Arc<dyn BrainModelClient>,
        listener_pool: PgPool,
    ) -> BrainConsumerBuilder {
        BrainConsumerBuilder {
            repos,
            outbox,
            model,
            listener_pool,
            profile: BrainProfile::default(),
            seams: PipelineSeams::default(),
            notifications: None,
        }
    }
}

impl BrainConsumerBuilder {
    #[must_use]
    pub fn profile(mut self, profile: BrainProfile) -> Self {
        self.profile = profile;
        self
    }

    #[must_use]
    pub fn seams(mut self, seams: PipelineSeams) -> Self {
        self.seams = seams;
        self
    }

    #[must_use]
    pub fn notifications(mut self, sink: DynNotificationSink) -> Self {
        self.notifications = Some(sink);
        self
    }

    #[must_use]
    pub fn build(self) -> BrainConsumer {
        let settings = self.profile.consumer;
        let pool = self.repos.write_pool().clone();
        let mut pipeline =
            BrainPipeline::builder(self.repos, self.model, self.profile).seams(self.seams);
        if let Some(sink) = self.notifications {
            pipeline = pipeline.notifications(sink);
        }
        BrainConsumer {
            pipeline: Arc::new(pipeline.build()),
            outbox: self.outbox,
            pool,
            listener_pool: self.listener_pool,
            settings,
            state: StateCell::default(),
            counters: Arc::new(Counters::default()),
        }
    }
}
