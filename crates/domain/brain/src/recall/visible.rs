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
//! that could not enumerate still goes to that resolver. Every membership
//! and source question a pass needs is asked once per distinct subject
//! (see `super::visible_memo`); the verdicts are the per-fact ones.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::brain::{RecallPrincipal, SourceAccess, WithheldSummary};
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_models::brain::{
    BrainFact, FactScope, FactSource, FactStatus, Granularity, ScoredFact, TopicDecl,
    WithheldReason,
};
use systemprompt_models::notifications::{AudienceSubject, SubjectRef};

use super::held::HeldKeys;
use super::visible_memo::{Answers, Questions};

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
    pub(super) source_access: Arc<dyn SourceAccess>,
    audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    topics: Vec<TopicDecl>,
    pub(super) source_timeout: Duration,
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
        let answers = Questions::collect(self, std::iter::once(fact), principal)
            .resolve(self, principal)
            .await;
        self.verdict(fact, principal, &answers)
    }

    pub async fn filter(
        &self,
        ranked: Vec<ScoredFact>,
        principal: &RecallPrincipal,
    ) -> (Vec<ScoredFact>, WithheldSummary) {
        let answers = Questions::collect(self, ranked.iter().map(|s| &s.fact), principal)
            .resolve(self, principal)
            .await;
        let mut withheld = WithheldSummary::default();
        let mut visible = Vec::with_capacity(ranked.len());
        for scored in ranked {
            match self.verdict(&scored.fact, principal, &answers) {
                Verdict::Visible => visible.push(scored),
                Verdict::Withheld(reason) => withheld.add(reason),
            }
        }
        (visible, withheld)
    }

    fn verdict(&self, fact: &BrainFact, principal: &RecallPrincipal, answers: &Answers) -> Verdict {
        if !self.scope_allows(&fact.scope, principal, answers)
            && !self.topic_allows(fact, principal, answers)
        {
            return Verdict::Withheld(WithheldReason::Scope);
        }
        if !self.granularity_allows(fact, principal) {
            return Verdict::Withheld(WithheldReason::Scope);
        }
        Self::source_allows(fact, principal, answers)
    }

    pub(super) fn audience_subjects(&self, fact: &BrainFact) -> Vec<AudienceSubject> {
        let mut subjects = Vec::new();
        if let FactScope::Group(key) = &fact.scope {
            subjects.push(group_subject(key));
        }
        for topic in &fact.topics {
            if let Some(decl) = self.topics.iter().find(|decl| &decl.id == topic) {
                subjects.extend(decl.audience.iter().cloned());
            }
        }
        subjects
    }

    pub(super) fn resolver_to_ask(
        &self,
        subject: &AudienceSubject,
    ) -> Option<&Arc<dyn NotificationAudienceResolver>> {
        if matches!(
            subject,
            AudienceSubject::Everyone | AudienceSubject::User(_)
        ) {
            return None;
        }
        if self.held.as_ref().is_some_and(|held| held.holds(subject)) {
            return None;
        }
        let resolver = self.audiences.iter().find(|r| r.handles(subject))?;
        let enumerated = self
            .held
            .as_ref()
            .is_some_and(|held| held.enumerated_by(resolver.name()));
        (!enumerated).then_some(resolver)
    }

    pub(super) fn source_to_ask(
        fact: &BrainFact,
        principal: &RecallPrincipal,
    ) -> Option<SubjectRef> {
        if private_until_confirmed(fact, principal) {
            return None;
        }
        fact.source.visibility_subject()
    }

    fn scope_allows(
        &self,
        scope: &FactScope,
        principal: &RecallPrincipal,
        answers: &Answers,
    ) -> bool {
        match scope {
            FactScope::User(owner) => owner == &principal.user_id,
            FactScope::Project(key) => principal.project.as_deref() == Some(key.as_str()),
            FactScope::Group(key) => self.is_member(&group_subject(key), principal, answers),
            FactScope::Org => true,
        }
    }

    fn topic_allows(
        &self,
        fact: &BrainFact,
        principal: &RecallPrincipal,
        answers: &Answers,
    ) -> bool {
        fact.topics.iter().any(|topic| {
            self.topics.iter().any(|decl| {
                &decl.id == topic
                    && decl
                        .audience
                        .iter()
                        .any(|subject| self.is_member(subject, principal, answers))
            })
        })
    }

    fn is_member(
        &self,
        subject: &AudienceSubject,
        principal: &RecallPrincipal,
        answers: &Answers,
    ) -> bool {
        match subject {
            AudienceSubject::Everyone => return true,
            AudienceSubject::User(user) => return user == &principal.user_id,
            AudienceSubject::Role(_) | AudienceSubject::Dimension { .. } => {},
        }
        if self.held.as_ref().is_some_and(|held| held.holds(subject)) {
            return true;
        }
        self.resolver_to_ask(subject).is_some() && answers.member(subject)
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

    fn source_allows(fact: &BrainFact, principal: &RecallPrincipal, answers: &Answers) -> Verdict {
        if private_until_confirmed(fact, principal) {
            return Verdict::Withheld(WithheldReason::Source);
        }
        fact.source
            .visibility_subject()
            .map_or(Verdict::Visible, |subject| answers.source(&subject))
    }
}

fn group_subject(key: &str) -> AudienceSubject {
    AudienceSubject::Dimension {
        kind: GROUP_DIMENSION.to_owned(),
        value: key.to_owned(),
    }
}

fn private_until_confirmed(fact: &BrainFact, principal: &RecallPrincipal) -> bool {
    let confirmed = fact.status == FactStatus::Confirmed;
    match &fact.source {
        FactSource::Session { owner, .. } => !confirmed && owner != &principal.user_id,
        FactSource::Run { .. } => !confirmed,
        _ => false,
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
