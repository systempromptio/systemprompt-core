//! `visible(fact, principal)`, decided per fact and failing closed:
//!
//! ```text
//! ( scope_allows || topic_audience_contains ) && granularity_allows && source_allows
//! ```
//!
//! A user-scoped fact is its owner's; a project fact needs the principal's
//! project; a group fact needs membership the injected audience resolvers
//! confirm; an org fact is everyone's. A topic audience widens scope and
//! never bypasses the source check. A person-subject fact under an
//! aggregate-only topic is shown only to that person. A source with a
//! visibility subject goes through [`SourceAccess`] under a timeout:
//! `Hidden`, an error or a timeout withholds it as `Source`, `AgentCapped`
//! as `AgentCapped`. Session and run facts stay private until confirmed.
//! Membership a resolver cannot decide is not membership.
//!
//! With the recall's [`HeldKeys`] attached, a role or dimension subject is
//! answered from the keys a resolver enumerated once per recall instead
//! of a membership query per fact; a subject handled only by a resolver
//! that could not enumerate still goes to that resolver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use systemprompt_extension::brain::{
    RecallPrincipal, SourceAccess, SourceVerdict, WithheldSummary,
};
use systemprompt_extension::notifications::{Membership, NotificationAudienceResolver};
use systemprompt_models::brain::{
    BrainFact, FactScope, FactSource, FactStatus, Granularity, ScoredFact, TopicDecl,
    WithheldReason,
};
use systemprompt_models::notifications::AudienceSubject;

use super::held::HeldKeys;

pub const GROUP_DIMENSION: &str = "group";
pub const PERSON_SUBJECT_KIND: &str = "user";

/// Whether one fact may reach the principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Visible,
    Withheld(WithheldReason),
}

/// The collaborators [`Visibility::check`] consults.
#[derive(Clone)]
pub struct Visibility {
    source_access: Arc<dyn SourceAccess>,
    audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    topics: Vec<TopicDecl>,
    source_timeout: Duration,
    held: Option<HeldKeys>,
}

impl std::fmt::Debug for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visibility")
            .field("audiences", &self.audiences.len())
            .field("topics", &self.topics.len())
            .field("source_timeout", &self.source_timeout)
            .field("held", &self.held)
            .finish_non_exhaustive()
    }
}

impl Visibility {
    #[must_use]
    pub fn new(source_access: Arc<dyn SourceAccess>, source_timeout: Duration) -> Self {
        Self {
            source_access,
            audiences: Vec::new(),
            topics: Vec::new(),
            source_timeout,
            held: None,
        }
    }

    #[must_use]
    pub fn with_held(mut self, held: HeldKeys) -> Self {
        self.held = Some(held);
        self
    }

    #[must_use]
    pub fn with_audiences(mut self, audiences: Vec<Arc<dyn NotificationAudienceResolver>>) -> Self {
        self.audiences = audiences;
        self
    }

    #[must_use]
    pub fn with_topics(mut self, topics: Vec<TopicDecl>) -> Self {
        self.topics = topics;
        self
    }

    pub async fn check(&self, fact: &BrainFact, principal: &RecallPrincipal) -> Verdict {
        if !self.scope_allows(&fact.scope, principal).await
            && !self.topic_allows(fact, principal).await
        {
            return Verdict::Withheld(WithheldReason::Scope);
        }
        if !self.granularity_allows(fact, principal) {
            return Verdict::Withheld(WithheldReason::Scope);
        }
        self.source_allows(fact, principal).await
    }

    pub async fn filter(
        &self,
        ranked: Vec<ScoredFact>,
        principal: &RecallPrincipal,
    ) -> (Vec<ScoredFact>, WithheldSummary) {
        let verdicts = join_all(
            ranked
                .iter()
                .map(|scored| self.check(&scored.fact, principal)),
        )
        .await;
        let mut withheld = WithheldSummary::default();
        let mut visible = Vec::with_capacity(ranked.len());
        for (scored, verdict) in ranked.into_iter().zip(verdicts) {
            match verdict {
                Verdict::Visible => visible.push(scored),
                Verdict::Withheld(reason) => withheld.add(reason),
            }
        }
        (visible, withheld)
    }

    async fn scope_allows(&self, scope: &FactScope, principal: &RecallPrincipal) -> bool {
        match scope {
            FactScope::User(owner) => owner == &principal.user_id,
            FactScope::Project(key) => principal.project.as_deref() == Some(key.as_str()),
            FactScope::Group(key) => {
                let group = AudienceSubject::Dimension {
                    kind: GROUP_DIMENSION.to_owned(),
                    value: key.clone(),
                };
                self.is_member(&group, principal).await
            },
            FactScope::Org => true,
        }
    }

    async fn topic_allows(&self, fact: &BrainFact, principal: &RecallPrincipal) -> bool {
        for topic in &fact.topics {
            let Some(decl) = self.topics.iter().find(|decl| &decl.id == topic) else {
                continue;
            };
            for subject in &decl.audience {
                if self.is_member(subject, principal).await {
                    return true;
                }
            }
        }
        false
    }

    async fn is_member(&self, subject: &AudienceSubject, principal: &RecallPrincipal) -> bool {
        match subject {
            AudienceSubject::Everyone => return true,
            AudienceSubject::User(user) => return user == &principal.user_id,
            AudienceSubject::Role(_) | AudienceSubject::Dimension { .. } => {},
        }
        if self.held.as_ref().is_some_and(|held| held.holds(subject)) {
            return true;
        }
        let Some(resolver) = self.audiences.iter().find(|r| r.handles(subject)) else {
            return false;
        };
        if self
            .held
            .as_ref()
            .is_some_and(|held| held.enumerated_by(resolver.name()))
        {
            return false;
        }
        matches!(
            resolver.is_member(subject, &principal.user_id).await,
            Ok(Membership::Member)
        )
    }

    fn granularity_allows(&self, fact: &BrainFact, principal: &RecallPrincipal) -> bool {
        if matches!(fact.source, FactSource::Aggregate { .. })
            || fact.subject.kind != PERSON_SUBJECT_KIND
            || fact.subject.id == principal.user_id.as_str()
        {
            return true;
        }
        !fact.topics.iter().any(|topic| {
            self.topics
                .iter()
                .any(|decl| &decl.id == topic && decl.granularity == Granularity::Aggregate)
        })
    }

    async fn source_allows(&self, fact: &BrainFact, principal: &RecallPrincipal) -> Verdict {
        let confirmed = fact.status == FactStatus::Confirmed;
        match &fact.source {
            FactSource::Session { owner, .. } if !confirmed && owner != &principal.user_id => {
                return Verdict::Withheld(WithheldReason::Source);
            },
            FactSource::Run { .. } if !confirmed => {
                return Verdict::Withheld(WithheldReason::Source);
            },
            _ => {},
        }
        let Some(subject) = fact.source.visibility_subject() else {
            return Verdict::Visible;
        };
        let check = self
            .source_access
            .readable(&subject, &principal.user_id, &principal.actor);
        match tokio::time::timeout(self.source_timeout, check).await {
            Ok(Ok(SourceVerdict::Readable)) => Verdict::Visible,
            Ok(Ok(SourceVerdict::AgentCapped)) => Verdict::Withheld(WithheldReason::AgentCapped),
            Ok(Ok(SourceVerdict::Hidden) | Err(_)) | Err(_) => {
                Verdict::Withheld(WithheldReason::Source)
            },
        }
    }
}

#[must_use]
pub fn lifecycle_withheld(
    fact: &BrainFact,
    now: chrono::DateTime<chrono::Utc>,
    inject_stale: bool,
) -> Option<WithheldReason> {
    if fact.status == FactStatus::Stale && !inject_stale {
        return Some(WithheldReason::Stale);
    }
    if fact.is_review_due(now) {
        return Some(WithheldReason::ReviewDue);
    }
    if fact.status == FactStatus::Disputed {
        return Some(WithheldReason::Disputed);
    }
    if !fact.is_valid_at(now)
        || matches!(
            fact.status,
            FactStatus::Candidate | FactStatus::Retired | FactStatus::Superseded
        )
    {
        return Some(WithheldReason::Scope);
    }
    None
}
