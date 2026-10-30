//! Planning and per-recipient decisions, shared by delivery and simulation.
//!
//! [`plan`] runs the routers, gates and expands the audience and loads the
//! recipients' own rules and the rule subjects' memberships; it only reads.
//! [`decide`] turns one planned recipient into a delivery or a withholding
//! without touching the database, in a fixed policy order: the routed
//! attention (core default, sender, extension routers), the stream floor and
//! ceiling, the recipient's rules, the stream ceiling again (a rule never
//! lifts an item above its stream's ceiling), the subscription ceiling, then
//! quiet hours. `attention_reason` names the step that changed the attention
//! last: a stream band, a rule, the subscription ceiling or quiet hours.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_identifiers::{NotificationRuleId, UserId};
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, DigestWindow, NotificationEvent,
    RecipientDecision, RoutePlan, RuleDecl, StreamDecl,
};

use super::audience::{SubscriptionConstraint, expand_audience};
use super::context::BatchContext;
use super::membership::Members;
use super::rules::{RuleContext, RuleEvaluator, in_quiet_hours};
use super::{StepFailure, route};
use crate::error::NotificationsResult;
use crate::repository::{FailureStage, NotificationRuleRepository};

pub(crate) struct Plan {
    pub(crate) routed: RoutePlan,
    pub(crate) stream: Option<StreamDecl>,
    pub(crate) constraints: HashMap<UserId, SubscriptionConstraint>,
    pub(crate) memberships: HashMap<UserId, Vec<AudienceSubject>>,
    pub(crate) in_audience: HashSet<UserId>,
    pub(crate) unresolved: Vec<AudienceSubject>,
    pub(crate) global_rules: Vec<RuleDecl>,
    pub(crate) owner_rules: HashMap<UserId, Vec<RuleDecl>>,
    pub(crate) failures: Vec<StepFailure>,
}

pub(crate) enum Decision {
    Deliver {
        attention: Attention,
        reason: AttentionReason,
        matched: Vec<NotificationRuleId>,
        coalesce_window_secs: Option<u32>,
        digest: Option<DigestWindow>,
    },
    Muted {
        matched: Vec<NotificationRuleId>,
    },
}

pub(crate) async fn plan(
    routers: &[std::sync::Arc<dyn systemprompt_extension::notifications::NotificationRouter>],
    members: &Members<'_>,
    conn: &mut PgConnection,
    ctx: &mut BatchContext,
    event: &NotificationEvent,
) -> NotificationsResult<Plan> {
    let (mut routed, mut failures) = route::route(routers, event, members.timeout).await?;
    let audience = expand_audience(&mut *conn, ctx, members, event, &mut routed).await?;
    failures.extend(audience.failures);
    routed.recipients.retain(|d| d.user != event.actor);
    let recipients: Vec<UserId> = routed.recipients.iter().map(|d| d.user.clone()).collect();
    let owner_rules = NotificationRuleRepository::list_for_owners(&mut *conn, &recipients).await?;
    let global_rules = ctx.global_rules().to_vec();
    let mut memberships = audience.memberships;
    let mut unresolved = Vec::new();
    let mut relevant: HashSet<&UserId> = recipients.iter().collect();
    relevant.insert(&event.actor);
    for subject in rule_subjects(global_rules.iter().chain(owner_rules.values().flatten())) {
        match ctx.rule_members(members, &subject).await {
            Ok(users) => {
                for user in users.iter().filter(|u| relevant.contains(u)) {
                    let known = memberships.entry(user.clone()).or_default();
                    if !known.contains(&subject) {
                        known.push(subject.clone());
                    }
                }
            },
            Err(withheld) => {
                failures.push(StepFailure::withheld(FailureStage::Rules, None, &withheld));
                unresolved.push(subject);
            },
        }
    }
    Ok(Plan {
        routed,
        stream: audience.stream,
        constraints: audience.constraints,
        memberships,
        in_audience: audience.in_audience,
        unresolved,
        global_rules,
        owner_rules,
        failures,
    })
}

fn rule_subjects<'r>(rules: impl Iterator<Item = &'r RuleDecl> + 'r) -> Vec<AudienceSubject> {
    let mut subjects: Vec<AudienceSubject> = Vec::new();
    for rule in rules {
        for subject in rule.r#match.actors.iter().chain(&rule.r#match.recipients) {
            let needs_members = matches!(
                subject,
                AudienceSubject::Role(_) | AudienceSubject::Dimension { .. }
            );
            if needs_members && !subjects.contains(subject) {
                subjects.push(subject.clone());
            }
        }
    }
    subjects
}

pub(crate) fn decide(
    event: &NotificationEvent,
    plan: &Plan,
    decision: &RecipientDecision,
    now: DateTime<Utc>,
) -> Decision {
    let user = &decision.user;
    let (attention, reason) = RuleEvaluator::stream_bands(
        decision.attention,
        decision.reason.clone(),
        plan.stream.as_ref(),
    );
    let rules: Vec<RuleDecl> = plan
        .global_rules
        .iter()
        .chain(plan.owner_rules.get(user).into_iter().flatten())
        .cloned()
        .collect();
    let no_subjects = Vec::new();
    let ctx = RuleContext {
        event,
        stream: plan.stream.as_ref(),
        recipient: user,
        recipient_subjects: plan.memberships.get(user).unwrap_or(&no_subjects),
        actor_subjects: plan.memberships.get(&event.actor).unwrap_or(&no_subjects),
        unresolved: &plan.unresolved,
        now,
    };
    let outcome = RuleEvaluator::evaluate(&rules, &ctx, attention);
    let Some(attention) = outcome.attention else {
        return Decision::Muted {
            matched: outcome.matched,
        };
    };
    let reason = outcome.changed_by.map_or(reason, AttentionReason::Rule);
    let stream_ceiling = plan.stream.as_ref().and_then(|stream| {
        stream
            .attention
            .ceiling
            .map(|ceiling| (ceiling, AttentionReason::StreamCeiling(stream.id.clone())))
    });
    let (attention, reason) = capped(attention, reason, stream_ceiling);
    let constraint = plan.constraints.get(user);
    let (attention, reason) = capped(
        attention,
        reason,
        constraint
            .and_then(|c| c.ceiling)
            .map(|ceiling| (ceiling, AttentionReason::SubscriptionCeiling)),
    );
    let quiet =
        outcome.quiet || constraint.is_some_and(|c| c.quiet.iter().any(|q| in_quiet_hours(q, now)));
    let (attention, reason) = capped(
        attention,
        reason,
        quiet.then_some((Attention::Agent, AttentionReason::QuietHours)),
    );
    Decision::Deliver {
        attention,
        reason,
        matched: outcome.matched,
        coalesce_window_secs: outcome
            .coalesce_window_secs
            .or_else(|| plan.stream.as_ref().and_then(|s| s.coalesce_window_secs)),
        digest: outcome.digest.or_else(|| constraint.and_then(|c| c.digest)),
    }
}

fn capped(
    attention: Attention,
    reason: AttentionReason,
    ceiling: Option<(Attention, AttentionReason)>,
) -> (Attention, AttentionReason) {
    match ceiling {
        Some((ceiling, cause)) if attention > ceiling => (ceiling, cause),
        _ => (attention, reason),
    }
}
