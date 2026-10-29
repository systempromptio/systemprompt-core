//! [`RuleEvaluator`]: the pure attention calculation for one recipient.
//!
//! Rules apply in order (declared rules by ascending priority, then the
//! recipient's own): `set` replaces, `cap` lowers to at most, `floor` raises
//! to at least, `mute` withholds and stops. A rule's quiet hours are
//! reported in [`RuleOutcome::quiet`] and applied by the caller as the last
//! policy step; [`RuleEvaluator::stream_bands`] is the step before the rules. A
//! rule matches when every non-empty predicate list matches; `kinds` are globs
//! over the kind string.
//!
//! Role and dimension predicates are answered from the subjects the pipeline
//! resolved the recipient (and the actor) into. A subject whose membership
//! could not be resolved is *unknown*: a rule that depends on it applies only
//! when applying it would not raise the recipient's attention, so an
//! unresolvable audience can make delivery quieter but never louder.
//!
//! Quiet hours take `tz` as `UTC` or a fixed `+HH:MM`/`-HH:MM` offset; any
//! other zone is treated as inside the window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Datelike, FixedOffset, Utc};
use globset::Glob;
use systemprompt_identifiers::{NotificationRuleId, UserId};
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, DigestWindow, NotificationEvent, QuietHours,
    RuleAction, RuleDecl, RuleMatch, StreamDecl,
};

/// Everything a rule may match against for one recipient of one event.
#[derive(Debug, Clone, Copy)]
pub struct RuleContext<'a> {
    pub event: &'a NotificationEvent,
    pub stream: Option<&'a StreamDecl>,
    pub recipient: &'a UserId,
    pub recipient_subjects: &'a [AudienceSubject],
    pub actor_subjects: &'a [AudienceSubject],
    pub unresolved: &'a [AudienceSubject],
    pub now: DateTime<Utc>,
}

/// The result of the rules: `attention: None` means muted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOutcome {
    pub attention: Option<Attention>,
    pub changed_by: Option<NotificationRuleId>,
    pub matched: Vec<NotificationRuleId>,
    pub coalesce_window_secs: Option<u32>,
    pub digest: Option<DigestWindow>,
    pub quiet: bool,
}

impl RuleOutcome {
    fn step(&mut self, rule: &RuleDecl, ctx: &RuleContext<'_>) {
        let Some(current) = self.attention else {
            return;
        };
        let verdict = rule_matches(&rule.r#match, ctx);
        let next = apply(&rule.action, current);
        let raises = next.is_some_and(|a| a > current);
        if verdict == Match::No || (verdict == Match::Unknown && raises) {
            return;
        }
        self.matched.push(rule.id.clone());
        if next != Some(current) {
            self.changed_by = Some(rule.id.clone());
        }
        self.attention = next;
        if verdict == Match::Yes && rule.action.coalesce_window_secs.is_some() {
            self.coalesce_window_secs = rule.action.coalesce_window_secs;
        }
        if verdict == Match::Yes && rule.action.digest.is_some() {
            self.digest = rule.action.digest;
        }
        let quiet = rule.action.quiet_hours.as_ref();
        self.quiet |= quiet.is_some_and(|q| in_quiet_hours(q, ctx.now));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Match {
    Yes,
    No,
    Unknown,
}

impl Match {
    const fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Yes,
        }
    }

    const fn from_bool(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

/// Stateless evaluator over a rule list.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuleEvaluator;

impl RuleEvaluator {
    #[must_use]
    pub fn evaluate(rules: &[RuleDecl], ctx: &RuleContext<'_>, decided: Attention) -> RuleOutcome {
        let mut outcome = RuleOutcome {
            attention: Some(decided),
            changed_by: None,
            matched: Vec::new(),
            coalesce_window_secs: None,
            digest: None,
            quiet: false,
        };
        for rule in rules {
            if outcome.attention.is_none() {
                break;
            }
            outcome.step(rule, ctx);
        }
        outcome
    }

    #[must_use]
    pub fn stream_bands(
        attention: Attention,
        reason: AttentionReason,
        stream: Option<&StreamDecl>,
    ) -> (Attention, AttentionReason) {
        let mut attention = attention;
        let mut reason = reason;
        if let Some(stream) = stream {
            if let Some(floor) = stream.attention.floor
                && attention < floor
            {
                attention = floor;
                reason = AttentionReason::StreamFloor(stream.id.clone());
            }
            if let Some(ceiling) = stream.attention.ceiling
                && attention > ceiling
            {
                attention = ceiling;
                reason = AttentionReason::StreamCeiling(stream.id.clone());
            }
        }
        (attention, reason)
    }
}

fn apply(action: &RuleAction, current: Attention) -> Option<Attention> {
    if action.mute {
        return None;
    }
    let mut next = action.set.unwrap_or(current);
    if let Some(cap) = action.cap {
        next = next.min(cap);
    }
    if let Some(floor) = action.floor {
        next = next.max(floor);
    }
    Some(next)
}

fn rule_matches(spec: &RuleMatch, ctx: &RuleContext<'_>) -> Match {
    let kind = ctx.event.kind.as_str();
    let kinds = spec.kinds.is_empty() || spec.kinds.iter().any(|p| kind_matches(p, kind));
    let streams = spec.streams.is_empty()
        || ctx
            .event
            .stream
            .as_ref()
            .is_some_and(|s| spec.streams.contains(s));
    let resources = spec.resources.is_empty()
        || spec
            .resources
            .iter()
            .any(|r| ctx.event.subjects.contains(r));
    let agent = spec
        .actor_is_agent
        .is_none_or(|expected| expected == ctx.event.actor_is_agent);
    Match::from_bool(kinds && streams && resources && agent)
        .and(subjects_match(
            &spec.actors,
            &ctx.event.actor,
            ctx.actor_subjects,
            ctx.unresolved,
        ))
        .and(subjects_match(
            &spec.recipients,
            ctx.recipient,
            ctx.recipient_subjects,
            ctx.unresolved,
        ))
}

fn kind_matches(pattern: &str, kind: &str) -> bool {
    Glob::new(pattern).map_or_else(
        |_| pattern == kind,
        |glob| glob.compile_matcher().is_match(kind),
    )
}

fn subjects_match(
    wanted: &[AudienceSubject],
    user: &UserId,
    known: &[AudienceSubject],
    unresolved: &[AudienceSubject],
) -> Match {
    if wanted.is_empty() {
        return Match::Yes;
    }
    let mut unknown = false;
    for subject in wanted {
        let hit = match subject {
            AudienceSubject::Everyone => true,
            AudienceSubject::User(id) => id == user,
            other => known.contains(other),
        };
        if hit {
            return Match::Yes;
        }
        unknown |= unresolved.contains(subject);
    }
    if unknown { Match::Unknown } else { Match::No }
}

#[must_use]
pub fn in_quiet_hours(quiet: &QuietHours, now: DateTime<Utc>) -> bool {
    let Some(offset) = fixed_offset(&quiet.tz) else {
        return true;
    };
    let local = now.with_timezone(&offset);
    if let Some(days) = &quiet.weekdays
        && !days.contains(&local.weekday())
    {
        return false;
    }
    let time = local.time();
    if quiet.start <= quiet.end {
        quiet.start <= time && time < quiet.end
    } else {
        time >= quiet.start || time < quiet.end
    }
}

fn fixed_offset(tz: &str) -> Option<FixedOffset> {
    if tz.eq_ignore_ascii_case("utc") || tz == "Z" {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match tz.as_bytes().first()? {
        b'+' => (1, &tz[1..]),
        b'-' => (-1, &tz[1..]),
        _ => return None,
    };
    let (hours, minutes) = rest.split_once(':')?;
    let hours: i32 = hours.parse().ok()?;
    let minutes: i32 = minutes.parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}
