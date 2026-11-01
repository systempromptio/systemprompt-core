//! [`BrainService`]: the person-facing write and read side of the brain.
//!
//! Entering a fact emits a `brain.fact.entered` event on its own
//! transaction for the consumer to build. Every lifecycle action loads the
//! fact, checks [`authority`] fail-closed, and changes it through
//! `FactRepository::apply` on one transaction, so each action is a
//! revision row attributed to the caller. Proposals are only decided here;
//! applying one is a separate step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authority;
mod disputes;
mod enter;
mod lifecycle;
mod proposals;
mod reads;
mod visibility;

use std::sync::{Arc, OnceLock};

use sqlx::PgConnection;
use systemprompt_extension::brain::{DynBrainSink, SourceAccess};
use systemprompt_extension::notifications::{DynNotificationSink, NotificationAudienceResolver};
use systemprompt_identifiers::BrainFactId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{BrainFact, FactRevision, RevisionChange};
use systemprompt_models::notifications::NotificationEvent;

pub use authority::BrainCaller;
pub use disputes::{DisputeOutcome, DisputeResolution, FACT_DISPUTED, FACT_SUBJECT};
pub use reads::FactWhy;

use crate::error::{BrainError, BrainResult};
use crate::models::FactChange;
use crate::outcomes;
use crate::recall::{DeclaredRecall, DeclaredSource, PlaneDeclaredSource};
use crate::replay::ProposalReplay;
use crate::repository::{BrainRepositories, FactRepository};

/// The brain's person-facing service.
#[derive(Clone)]
pub struct BrainService {
    repos: BrainRepositories,
    sink: DynBrainSink,
    notifications: Option<DynNotificationSink>,
    profile: Arc<BrainProfile>,
    declared: Arc<dyn DeclaredSource>,
    source_access: Arc<dyn SourceAccess>,
    audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    replay: Arc<OnceLock<ProposalReplay>>,
}

impl std::fmt::Debug for BrainService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainService")
            .field("enabled", &self.profile.enabled)
            .field("notifications", &self.notifications.is_some())
            .field("replay", &self.replay.get().is_some())
            .finish_non_exhaustive()
    }
}

/// Builds a [`BrainService`]; the declarations default to the plane tables.
pub struct BrainServiceBuilder {
    repos: BrainRepositories,
    sink: DynBrainSink,
    profile: Arc<BrainProfile>,
    notifications: Option<DynNotificationSink>,
    declared: Option<Arc<dyn DeclaredSource>>,
    source_access: Option<Arc<dyn SourceAccess>>,
    audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    replay: Option<ProposalReplay>,
}

impl std::fmt::Debug for BrainServiceBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainServiceBuilder")
            .field("notifications", &self.notifications.is_some())
            .field("declared", &self.declared.is_some())
            .finish_non_exhaustive()
    }
}

impl BrainService {
    #[must_use]
    pub fn builder(
        repos: BrainRepositories,
        sink: DynBrainSink,
        profile: Arc<BrainProfile>,
    ) -> BrainServiceBuilder {
        BrainServiceBuilder {
            repos,
            sink,
            profile,
            notifications: None,
            declared: None,
            source_access: None,
            audiences: Vec::new(),
            replay: None,
        }
    }

    #[must_use]
    pub const fn profile(&self) -> &Arc<BrainProfile> {
        &self.profile
    }

    pub fn attach_replay(&self, replay: ProposalReplay) -> bool {
        self.replay.set(replay).is_ok()
    }
}

impl BrainServiceBuilder {
    #[must_use]
    pub fn notifications(mut self, sink: DynNotificationSink) -> Self {
        self.notifications = Some(sink);
        self
    }

    #[must_use]
    pub fn declared(mut self, declared: Arc<dyn DeclaredSource>) -> Self {
        self.declared = Some(declared);
        self
    }

    #[must_use]
    pub fn source_access(mut self, source_access: Arc<dyn SourceAccess>) -> Self {
        self.source_access = Some(source_access);
        self
    }

    #[must_use]
    pub fn audiences(mut self, audiences: Vec<Arc<dyn NotificationAudienceResolver>>) -> Self {
        self.audiences = audiences;
        self
    }

    #[must_use]
    pub fn replay(mut self, replay: ProposalReplay) -> Self {
        self.replay = Some(replay);
        self
    }

    #[must_use]
    pub fn build(self) -> BrainService {
        let declared = self
            .declared
            .unwrap_or_else(|| Arc::new(PlaneDeclaredSource::new(self.repos.clone())));
        BrainService {
            repos: self.repos,
            sink: self.sink,
            notifications: self.notifications,
            profile: self.profile,
            declared,
            source_access: self
                .source_access
                .unwrap_or_else(|| Arc::new(visibility::NoSourceAccess)),
            audiences: self.audiences,
            replay: Arc::new(self.replay.map_or_else(OnceLock::new, OnceLock::from)),
        }
    }
}

impl BrainService {
    async fn govern<F>(
        &self,
        caller: &BrainCaller,
        id: &BrainFactId,
        change: F,
    ) -> BrainResult<FactRevision>
    where
        F: FnOnce(&BrainFact, &DeclaredRecall) -> BrainResult<FactChange> + Send,
    {
        let declared = self.declared.load().await?;
        let mut tx = self.repos.write_pool().begin().await?;
        let fact = Self::fact_on(&mut tx, id).await?;
        authority::may_govern(caller, &fact.scope, &fact.kind, &declared.workflows)?;
        let change = change(&fact, &declared)?;
        let action = change.revision_change();
        let revision = FactRepository::apply(&mut tx, id, change).await?;
        if matches!(
            action,
            RevisionChange::Retracted | RevisionChange::FieldsEdited
        ) {
            let window = self.profile.lifecycle.correction_window_days;
            outcomes::on_correction(&mut *tx, id, &caller.user_id, window).await?;
        }
        tx.commit().await?;
        tracing::info!(
            fact_id = %id,
            actor = %caller.user_id,
            action = action.as_str(),
            revision = revision.revision,
            "Brain fact changed"
        );
        Ok(revision)
    }

    async fn fact_on(conn: &mut PgConnection, id: &BrainFactId) -> BrainResult<BrainFact> {
        FactRepository::find_on(conn, id)
            .await?
            .ok_or_else(|| BrainError::NotFound(id.clone()))
    }

    async fn notify(&self, conn: &mut PgConnection, event: &NotificationEvent) -> BrainResult<()> {
        let Some(sink) = self
            .notifications
            .as_ref()
            .filter(|sink| sink.is_available())
        else {
            tracing::debug!(
                kind = event.kind.as_str(),
                "Notifications unavailable; brain notice skipped"
            );
            return Ok(());
        };
        sink.emit(conn, event).await.map_err(BrainError::Notify)
    }
}
