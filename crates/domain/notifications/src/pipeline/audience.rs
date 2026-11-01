//! The audience step: a stream's declared audience, its subscribers and the
//! subscribers of the event's resources, resolved to users.
//!
//! A stream's audience is a gate, not only a list: a subscriber outside the
//! expanded declared audience is admitted only when a resolver answers
//! [`Membership::Member`](systemprompt_extension::notifications::Membership)
//! for one of the stream's audience subjects; `NotMember` and `Unknown`
//! withhold that subscriber and record why. A subject that cannot be
//! expanded (no resolver, a failure, a timeout, more members than the limit)
//! is withheld whole and recorded once. Resource subscribers are not
//! audience-gated; visibility decides them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use sqlx::PgConnection;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, DigestWindow, NotificationEvent, QuietHours,
    RecipientDecision, RoutePlan, StreamDecl,
};

use super::StepFailure;
use super::context::BatchContext;
use super::membership::Members;
use crate::error::NotificationsResult;
use crate::models::SubscriptionView;
use crate::repository::{FailureStage, NotificationSubscriptionRepository};

#[derive(Debug, Clone, Default)]
pub(crate) struct SubscriptionConstraint {
    pub(crate) ceiling: Option<Attention>,
    pub(crate) quiet: Vec<QuietHours>,
    pub(crate) digest: Option<DigestWindow>,
}

impl SubscriptionConstraint {
    fn tighten(&mut self, view: &SubscriptionView) {
        if let Some(ceiling) = view.decl.attention_ceiling {
            self.ceiling = Some(self.ceiling.map_or(ceiling, |c| c.min(ceiling)));
        }
        if let Some(digest) = view.decl.digest {
            self.digest = Some(match self.digest {
                Some(held) if held.every_secs <= digest.every_secs => held,
                _ => digest,
            });
        }
        if let Some(quiet) = &view.decl.quiet_hours {
            self.quiet.push(quiet.clone());
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct AudienceOutcome {
    pub(crate) stream: Option<StreamDecl>,
    pub(crate) constraints: HashMap<UserId, SubscriptionConstraint>,
    pub(crate) memberships: HashMap<UserId, Vec<AudienceSubject>>,
    pub(crate) in_audience: HashSet<UserId>,
    pub(crate) failures: Vec<StepFailure>,
}

impl AudienceOutcome {
    fn reach(&mut self, user: &UserId, subject: &AudienceSubject, view: Option<&SubscriptionView>) {
        if !matches!(subject, AudienceSubject::User(_)) {
            let known = self.memberships.entry(user.clone()).or_default();
            if !known.contains(subject) {
                known.push(subject.clone());
            }
        }
        if let Some(view) = view {
            self.constraints
                .entry(user.clone())
                .or_default()
                .tighten(view);
        }
    }
}

pub(crate) async fn expand_audience(
    conn: &mut PgConnection,
    ctx: &mut BatchContext,
    members: &Members<'_>,
    event: &NotificationEvent,
    plan: &mut RoutePlan,
) -> NotificationsResult<AudienceOutcome> {
    let mut outcome = AudienceOutcome::default();
    let stream_id = event.stream.clone().or_else(|| plan.stream.clone());
    let mut reached: Vec<UserId> = Vec::new();
    if let Some(id) = &stream_id {
        match ctx.stream(id).cloned() {
            Some(stream) => {
                let subscriptions = ctx.stream_subscriptions(&mut *conn, id).await?;
                gate_stream(members, &stream, &subscriptions, &mut outcome).await;
                reached.extend(outcome.in_audience.iter().cloned());
                outcome.stream = Some(stream);
            },
            None => outcome.failures.push(StepFailure::new(
                FailureStage::Audience,
                None,
                format!("unknown_stream {id}"),
            )),
        }
    }
    for view in
        NotificationSubscriptionRepository::list_for_resources(&mut *conn, &event.subjects).await?
    {
        match members.expand(&view.decl.subject).await {
            Ok(users) => {
                for user in users {
                    outcome.reach(&user, &view.decl.subject, Some(&view));
                    reached.push(user);
                }
            },
            Err(withheld) => outcome.failures.push(StepFailure::withheld(
                FailureStage::Audience,
                None,
                &withheld,
            )),
        }
    }
    let default = outcome
        .stream
        .as_ref()
        .map_or(Attention::Agent, |s| s.attention.default);
    let mut seen = HashSet::new();
    let recipients = reached
        .into_iter()
        .filter(|user| seen.insert(user.clone()))
        .map(|user| RecipientDecision {
            user,
            attention: default,
            reason: AttentionReason::Default,
        })
        .collect();
    plan.merge(RoutePlan {
        recipients,
        stream: stream_id,
    });
    Ok(outcome)
}

async fn gate_stream(
    members: &Members<'_>,
    stream: &StreamDecl,
    subscriptions: &[SubscriptionView],
    outcome: &mut AudienceOutcome,
) {
    let mut declared: HashSet<UserId> = HashSet::new();
    for subject in &stream.audience {
        match members.expand(subject).await {
            Ok(users) => {
                for user in users {
                    outcome.reach(&user, subject, None);
                    declared.insert(user);
                }
            },
            Err(withheld) => outcome.failures.push(StepFailure::withheld(
                FailureStage::Audience,
                None,
                &withheld,
            )),
        }
    }
    outcome.in_audience.extend(declared.iter().cloned());
    for view in subscriptions {
        let users = match members.expand(&view.decl.subject).await {
            Ok(users) => users,
            Err(withheld) => {
                outcome.failures.push(StepFailure::withheld(
                    FailureStage::Audience,
                    None,
                    &withheld,
                ));
                continue;
            },
        };
        for user in users {
            if !declared.contains(&user)
                && let Err(withheld) = members.gate(&stream.audience, &user).await
            {
                outcome.failures.push(StepFailure::withheld(
                    FailureStage::Audience,
                    Some(user),
                    &withheld,
                ));
                continue;
            }
            outcome.reach(&user, &view.decl.subject, Some(view));
            outcome.in_audience.insert(user);
        }
    }
}
