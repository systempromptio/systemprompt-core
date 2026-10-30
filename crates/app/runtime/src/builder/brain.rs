//! Composition of the [`BrainHandle`] from the context's handles.
//!
//! The brain sink exists before the document store, which emits through
//! it, so [`BrainCore`] is built first and only when the profile enables
//! the brain; a disabled brain composes nothing and the store keeps the
//! unavailable sink. Recall's extension seams take the built context, so,
//! as for notifications, the builder assembles the handle twice from one
//! core: with the core audience resolver alone, then with the discovered
//! audience resolvers, retrievers and injection policies ahead of it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_brain::recall::{
    BrainRecallService, BrainRecallStore, DeclaredSource, PlaneDeclaredSource,
};
use systemprompt_brain::service::BrainService;
use systemprompt_brain::{BrainRepositories, OutboxBrainSink};
use systemprompt_database::DbPool;
use systemprompt_events::services::durable::{DurableOutbox, OutboxConsumer};
use systemprompt_extension::ExtensionContext;
use systemprompt_extension::brain::{
    DynBrainSink, FactRetriever, ImportanceModel, InjectionPolicy,
    discover_brain_importance_models, discover_brain_injection_policies, discover_brain_retrievers,
};
use systemprompt_extension::notifications::{
    DynNotificationSink, NotificationAudienceResolver, discover_notification_audiences,
};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_users::UserRepository;

use crate::brain::{BrainHandle, BrainHandleParts, DocumentSourceAccess, DocumentSourceContent};
use crate::documents::{BrainUnavailableSink, DocumentsRuntime};
use crate::notifications::UserRoleAudienceResolver;

/// The context handles a [`BrainCore`] is built from.
#[derive(Clone, Copy)]
pub struct BrainInputs<'a> {
    pub database: &'a DbPool,
    pub instance_id: &'a InstanceId,
    pub listener_pool: &'a PgPool,
    pub notifications: &'a DynNotificationSink,
    pub profile: &'a BrainProfile,
}

impl fmt::Debug for BrainInputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainInputs")
            .field("enabled", &self.profile.enabled)
            .finish_non_exhaustive()
    }
}

/// The extension seams recall composes ahead of the core audience resolver.
#[derive(Default)]
pub struct RecallSeams {
    pub audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub retrievers: Vec<Arc<dyn FactRetriever>>,
    pub injection: Vec<Arc<dyn InjectionPolicy>>,
    pub importance: Vec<Arc<dyn ImportanceModel>>,
}

impl fmt::Debug for RecallSeams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecallSeams")
            .field("audiences", &self.audiences.len())
            .field("retrievers", &self.retrievers.len())
            .field("injection", &self.injection.len())
            .field("importance", &self.importance.len())
            .finish()
    }
}

impl RecallSeams {
    #[must_use]
    pub fn discover(ctx: &dyn ExtensionContext) -> Self {
        Self {
            audiences: discover_notification_audiences()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            retrievers: discover_brain_retrievers()
                .iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            injection: discover_brain_injection_policies()
                .iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            importance: discover_brain_importance_models()
                .iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
        }
    }
}

/// The brain's context-independent parts, built once per context.
pub struct BrainCore {
    repos: Arc<BrainRepositories>,
    sink: DynBrainSink,
    declared: Arc<dyn DeclaredSource>,
    service: Arc<BrainService>,
    users: Arc<UserRepository>,
    outbox: Arc<OutboxConsumer>,
    listener_pool: PgPool,
    notifications: DynNotificationSink,
    profile: Arc<BrainProfile>,
}

impl fmt::Debug for BrainCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainCore").finish_non_exhaustive()
    }
}

impl BrainCore {
    #[must_use]
    pub fn new(inputs: &BrainInputs<'_>) -> Option<Self> {
        if !inputs.profile.enabled {
            return None;
        }
        let write_pool = inputs.database.write_pool().as_ref().clone();
        let repos = BrainRepositories::new(inputs.database);
        let outbox = DurableOutbox::new(write_pool.clone(), inputs.instance_id.clone());
        let sink: DynBrainSink = Arc::new(OutboxBrainSink::new(Arc::new(outbox)));
        let profile = Arc::new(inputs.profile.clone());
        let declared: Arc<dyn DeclaredSource> = Arc::new(PlaneDeclaredSource::new(repos.clone()));
        let service = BrainService::builder(repos.clone(), Arc::clone(&sink), Arc::clone(&profile))
            .notifications(Arc::clone(inputs.notifications))
            .declared(Arc::clone(&declared))
            .build();
        Some(Self {
            repos: Arc::new(repos),
            sink,
            declared,
            service: Arc::new(service),
            users: Arc::new(UserRepository::new(inputs.database)),
            outbox: Arc::new(OutboxConsumer::new(write_pool)),
            listener_pool: inputs.listener_pool.clone(),
            notifications: Arc::clone(inputs.notifications),
            profile,
        })
    }

    #[must_use]
    pub fn sink(&self) -> DynBrainSink {
        Arc::clone(&self.sink)
    }

    #[must_use]
    pub fn assemble(&self, documents: &DocumentsRuntime, seams: RecallSeams) -> Arc<BrainHandle> {
        let RecallSeams {
            mut audiences,
            retrievers,
            injection,
            importance: _,
        } = seams;
        audiences.push(Arc::new(UserRoleAudienceResolver::new(Arc::clone(
            &self.users,
        ))));
        let store = BrainRecallStore::new((*self.repos).clone(), self.profile.recall.stale);
        let source_access = DocumentSourceAccess::new(
            Arc::clone(documents.access()),
            Arc::clone(documents.repositories()),
        );
        let recall = BrainRecallService::builder(
            Arc::new(store),
            Arc::new(source_access),
            Arc::clone(&self.declared),
            Arc::clone(&self.profile),
        )
        .retrievers(retrievers)
        .injection_policies(injection)
        .audiences(audiences)
        .build();
        let source_content = DocumentSourceContent::new(
            Arc::clone(documents.service()),
            Arc::clone(documents.repositories()),
        );
        Arc::new(BrainHandle::new(BrainHandleParts {
            repos: Arc::clone(&self.repos),
            service: Arc::clone(&self.service),
            recall: Arc::new(recall),
            sink: Arc::clone(&self.sink),
            notifications: Arc::clone(&self.notifications),
            source_content: Arc::new(source_content),
            outbox: Arc::clone(&self.outbox),
            listener_pool: self.listener_pool.clone(),
            profile: Arc::clone(&self.profile),
        }))
    }
}

#[must_use]
pub fn brain_sink(core: Option<&BrainCore>) -> DynBrainSink {
    match core {
        Some(core) => core.sink(),
        None => Arc::new(BrainUnavailableSink),
    }
}
