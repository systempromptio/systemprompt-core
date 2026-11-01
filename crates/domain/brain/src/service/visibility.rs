//! The read side's visibility: the same per-fact check recall applies,
//! for the caller's actor and project. A fact the caller may not see is
//! reported as not found, so its existence does not leak. A service built
//! without a source access check reads no sourced fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::brain::{
    RecallPrincipal, SourceAccess, SourceAccessError, SourceVerdict,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::BrainFact;
use systemprompt_models::notifications::SubjectRef;
use systemprompt_models::origin::ClientKind;

use super::{BrainCaller, BrainService};
use crate::error::{BrainError, BrainResult};
use crate::recall::visible::{Verdict, Visibility};

#[derive(Debug, Clone, Copy)]
pub(super) struct NoSourceAccess;

#[async_trait::async_trait]
impl SourceAccess for NoSourceAccess {
    async fn readable(
        &self,
        _subject: &SubjectRef,
        _reader: &UserId,
        _actor: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        Ok(SourceVerdict::Hidden)
    }
}

fn principal(caller: &BrainCaller) -> RecallPrincipal {
    RecallPrincipal {
        user_id: caller.user_id.clone(),
        client: ClientKind::Unknown,
        actor: caller.actor.clone(),
        project: caller.project.clone(),
        client_id: None,
    }
}

impl BrainService {
    pub(super) async fn visibility(&self) -> BrainResult<Visibility> {
        let declared = self.declared.load().await?;
        let timeout = Duration::from_millis(self.profile.consumer.source_timeout_ms);
        Ok(Visibility::new(Arc::clone(&self.source_access), timeout)
            .with_audiences(self.audiences.clone())
            .with_topics(declared.topics.clone()))
    }

    pub(super) async fn require_visible(
        &self,
        caller: &BrainCaller,
        fact: BrainFact,
    ) -> BrainResult<BrainFact> {
        let visibility = self.visibility().await?;
        match visibility.check(&fact, &principal(caller)).await {
            Verdict::Visible => Ok(fact),
            Verdict::Withheld(_) => Err(BrainError::NotFound(fact.id)),
        }
    }

    pub(super) async fn only_visible(
        &self,
        caller: &BrainCaller,
        facts: Vec<BrainFact>,
    ) -> BrainResult<Vec<BrainFact>> {
        let visibility = self.visibility().await?;
        let who = principal(caller);
        let verdicts =
            futures::future::join_all(facts.iter().map(|fact| visibility.check(fact, &who))).await;
        Ok(facts
            .into_iter()
            .zip(verdicts)
            .filter_map(|(fact, verdict)| (verdict == Verdict::Visible).then_some(fact))
            .collect())
    }
}
