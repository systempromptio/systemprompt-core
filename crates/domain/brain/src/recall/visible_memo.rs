//! The questions one visibility pass asks, each asked once.
//!
//! `visible(fact, principal)` turns on two kinds of answer that depend
//! only on the subject within one recall: whether the principal is a
//! member of an audience subject (a group scope, a topic audience), and
//! whether a source subject (a document, an external reference) is
//! readable by the principal. [`Questions`] collects the distinct subjects
//! a set of facts would ask about, leaving out every one the rules answer
//! without a call (an `Everyone` or `User` subject, a held key, a subject
//! no resolver handles, one an enumerating resolver did not list, a source
//! the session or run rule already decides), and [`Answers`] resolves them
//! concurrently, each source check under the recall's timeout. The
//! per-fact verdict is then decided from the answers in rank order, so a
//! document ten facts cite is checked once and every fact gets the verdict
//! the per-fact check would have given it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use futures::future::join_all;
use systemprompt_extension::brain::{RecallPrincipal, SourceVerdict, WithheldReason};
use systemprompt_extension::notifications::{Membership, NotificationAudienceResolver};
use systemprompt_models::brain::BrainFact;
use systemprompt_models::notifications::{AudienceSubject, SubjectRef};

use super::held::key_for;
use super::visible::{Verdict, Visibility};

#[derive(Default)]
pub(super) struct Questions<'a> {
    members: BTreeMap<String, (AudienceSubject, &'a Arc<dyn NotificationAudienceResolver>)>,
    sources: HashSet<SubjectRef>,
}

#[derive(Debug, Default)]
pub(super) struct Answers {
    members: HashMap<String, bool>,
    sources: HashMap<SubjectRef, Verdict>,
}

impl Answers {
    pub(super) fn member(&self, subject: &AudienceSubject) -> bool {
        self.members
            .get(&key_for(subject))
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn source(&self, subject: &SubjectRef) -> Verdict {
        self.sources
            .get(subject)
            .copied()
            .unwrap_or(Verdict::Withheld(WithheldReason::Source))
    }
}

impl<'a> Questions<'a> {
    pub(super) fn collect(
        visibility: &'a Visibility,
        facts: &mut dyn Iterator<Item = &BrainFact>,
        principal: &RecallPrincipal,
    ) -> Self {
        let mut questions = Self::default();
        for fact in facts {
            for subject in visibility.audience_subjects(fact) {
                if let Some(resolver) = visibility.resolver_to_ask(&subject) {
                    questions
                        .members
                        .insert(key_for(&subject), (subject, resolver));
                }
            }
            if let Some(subject) = Visibility::source_to_ask(fact, principal) {
                questions.sources.insert(subject);
            }
        }
        questions
    }

    pub(super) async fn resolve(
        self,
        visibility: &Visibility,
        principal: &RecallPrincipal,
    ) -> Answers {
        let members = join_all(self.members.into_iter().map(
            |(key, (subject, resolver))| async move {
                let member = matches!(
                    resolver.is_member(&subject, &principal.user_id).await,
                    Ok(Membership::Member)
                );
                (key, member)
            },
        ));
        let sources = join_all(self.sources.into_iter().map(|subject| async move {
            let verdict = visibility.readable(&subject, principal).await;
            (subject, verdict)
        }));
        let (members, sources) = futures::join!(members, sources);
        Answers {
            members: members.into_iter().collect(),
            sources: sources.into_iter().collect(),
        }
    }
}

impl Visibility {
    async fn readable(&self, subject: &SubjectRef, principal: &RecallPrincipal) -> Verdict {
        let check = self
            .source_access
            .readable(subject, &principal.user_id, &principal.actor);
        match tokio::time::timeout(self.source_timeout, check).await {
            Ok(Ok(SourceVerdict::Readable)) => Verdict::Visible,
            Ok(Ok(SourceVerdict::AgentCapped)) => Verdict::Withheld(WithheldReason::AgentCapped),
            Ok(Ok(SourceVerdict::Hidden) | Err(_)) | Err(_) => {
                Verdict::Withheld(WithheldReason::Source)
            },
        }
    }
}
