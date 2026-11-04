//! [`BrainRecallServiceBuilder`]: assembles a [`BrainRecallService`] from
//! its store, the loader that reads the fused facts (and the facts only an
//! extension retriever finds), source check, declarations and profile,
//! with optional extension retrievers, injection policies, importance
//! models and audience resolvers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::brain::{
    FactRecallSource, FactRetriever, ImportanceModel, InjectionPolicy, SourceAccess,
};
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_manifest::profile::BrainProfile;

use super::declared::DeclaredSource;
use super::introduce::FactLoader;
use super::service::BrainRecallService;
use crate::outcomes::OutcomeService;

impl std::fmt::Debug for BrainRecallService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainRecallService")
            .field("retrievers", &self.retrievers.len())
            .field("injection", &self.injection.len())
            .field("importance", &self.importance.len())
            .field("audiences", &self.audiences.len())
            .field("outcomes", &self.outcomes.is_some())
            .finish_non_exhaustive()
    }
}

/// Builds a [`BrainRecallService`].
#[derive(Debug)]
pub struct BrainRecallServiceBuilder {
    service: BrainRecallService,
}

impl BrainRecallService {
    #[must_use]
    pub fn builder(
        store: Arc<dyn FactRecallSource>,
        loader: Arc<dyn FactLoader>,
        source_access: Arc<dyn SourceAccess>,
        declared: Arc<dyn DeclaredSource>,
        profile: Arc<BrainProfile>,
    ) -> BrainRecallServiceBuilder {
        let source_timeout = Duration::from_millis(profile.recall.source_timeout_ms);
        BrainRecallServiceBuilder {
            service: Self {
                store,
                source_access,
                declared,
                profile,
                retrievers: Vec::new(),
                loader,
                injection: Vec::new(),
                importance: Vec::new(),
                audiences: Vec::new(),
                source_timeout,
                outcomes: None,
            },
        }
    }
}

impl BrainRecallServiceBuilder {
    #[must_use]
    pub fn retrievers(mut self, retrievers: Vec<Arc<dyn FactRetriever>>) -> Self {
        self.service.retrievers = retrievers;
        self
    }

    #[must_use]
    pub fn injection_policies(mut self, policies: Vec<Arc<dyn InjectionPolicy>>) -> Self {
        self.service.injection = policies;
        self
    }

    #[must_use]
    pub fn importance(mut self, models: Vec<Arc<dyn ImportanceModel>>) -> Self {
        self.service.importance = models;
        self
    }

    #[must_use]
    pub fn audiences(mut self, audiences: Vec<Arc<dyn NotificationAudienceResolver>>) -> Self {
        self.service.audiences = audiences;
        self
    }

    #[must_use]
    pub const fn source_timeout(mut self, timeout: Duration) -> Self {
        self.service.source_timeout = timeout;
        self
    }

    #[must_use]
    pub fn outcomes(mut self, outcomes: OutcomeService) -> Self {
        self.service.outcomes = Some(outcomes);
        self
    }

    #[must_use]
    pub fn build(self) -> BrainRecallService {
        self.service
    }
}
