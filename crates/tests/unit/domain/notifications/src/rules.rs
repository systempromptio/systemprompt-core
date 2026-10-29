use chrono::NaiveTime;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, DigestWindow, NotificationEvent, QuietHours,
    RuleAction, RuleDecl, RuleMatch, StreamAttention, StreamDecl, SubjectRef,
};
use systemprompt_notifications::pipeline::{RuleContext, RuleEvaluator, RuleOutcome};

use crate::fixtures::{any, at, cap, event, floor, mute, rule, set, stream, stream_id, utc};

struct Case<'a> {
    event: NotificationEvent,
    stream: Option<&'a StreamDecl>,
    recipient: UserId,
    recipient_subjects: Vec<AudienceSubject>,
    actor_subjects: Vec<AudienceSubject>,
    unresolved: Vec<AudienceSubject>,
    now: chrono::DateTime<chrono::Utc>,
}

impl Case<'_> {
    fn new(kind: &str) -> Self {
        Self {
            event: event(kind),
            stream: None,
            recipient: UserId::new("reader"),
            recipient_subjects: Vec::new(),
            actor_subjects: Vec::new(),
            unresolved: Vec::new(),
            now: at(1_000_000),
        }
    }

    fn evaluate(&self, rules: &[RuleDecl], decided: Attention) -> RuleOutcome {
        let ctx = RuleContext {
            event: &self.event,
            stream: self.stream,
            recipient: &self.recipient,
            recipient_subjects: &self.recipient_subjects,
            actor_subjects: &self.actor_subjects,
            unresolved: &self.unresolved,
            now: self.now,
        };
        RuleEvaluator::evaluate(rules, &ctx, decided)
    }
}

fn kinds(patterns: &[&str]) -> RuleMatch {
    RuleMatch {
        kinds: patterns.iter().map(|p| (*p).to_owned()).collect(),
        ..RuleMatch::default()
    }
}

fn recipients(subjects: &[AudienceSubject]) -> RuleMatch {
    RuleMatch {
        recipients: subjects.to_vec(),
        ..RuleMatch::default()
    }
}

fn role(name: &str) -> AudienceSubject {
    AudienceSubject::Role(name.to_owned())
}

#[test]
fn no_rules_keep_the_routed_attention() {
    let outcome = Case::new("document.shared").evaluate(&[], Attention::Agent);
    assert_eq!(outcome.attention, Some(Attention::Agent));
    assert_eq!(outcome.changed_by, None);
    assert!(outcome.matched.is_empty());
    assert!(!outcome.quiet);
}

#[test]
fn rules_apply_in_the_order_given_and_the_last_change_is_credited() {
    let first = rule(1, any(), set(Attention::Human));
    let second = rule(2, any(), set(Attention::Silent));
    let case = Case::new("document.shared");

    let outcome = case.evaluate(&[first.clone(), second.clone()], Attention::Agent);
    assert_eq!(outcome.attention, Some(Attention::Silent));
    assert_eq!(outcome.changed_by, Some(second.id.clone()));
    assert_eq!(outcome.matched, vec![first.id.clone(), second.id.clone()]);

    let reversed = case.evaluate(&[second, first.clone()], Attention::Agent);
    assert_eq!(reversed.attention, Some(Attention::Human));
    assert_eq!(reversed.changed_by, Some(first.id));
}

#[test]
fn a_matching_rule_that_changes_nothing_is_matched_but_not_credited() {
    let noop = rule(1, any(), cap(Attention::Human));
    let outcome =
        Case::new("document.shared").evaluate(std::slice::from_ref(&noop), Attention::Agent);
    assert_eq!(outcome.attention, Some(Attention::Agent));
    assert_eq!(outcome.matched, vec![noop.id]);
    assert_eq!(outcome.changed_by, None);
}

#[test]
fn kind_predicates_are_globs_over_the_kind() {
    let case = Case::new("document.comment.mentioned");
    let glob = rule(1, kinds(&["document.*"]), set(Attention::Silent));
    assert_eq!(
        case.evaluate(&[glob], Attention::Human).attention,
        Some(Attention::Silent)
    );
    let other = rule(
        1,
        kinds(&["deal.*", "stream.published"]),
        set(Attention::Silent),
    );
    assert_eq!(
        case.evaluate(&[other], Attention::Human).attention,
        Some(Attention::Human)
    );
    let exact = rule(
        1,
        kinds(&["document.comment.mentioned"]),
        set(Attention::Agent),
    );
    assert_eq!(
        case.evaluate(&[exact], Attention::Human).attention,
        Some(Attention::Agent)
    );
}

#[test]
fn an_unparsable_glob_falls_back_to_an_exact_match() {
    let broken = rule(1, kinds(&["document.[shared"]), set(Attention::Silent));
    assert_eq!(
        Case::new("document.shared")
            .evaluate(std::slice::from_ref(&broken), Attention::Agent)
            .attention,
        Some(Attention::Agent)
    );
    assert_eq!(
        Case::new("document.[shared")
            .evaluate(&[broken], Attention::Agent)
            .attention,
        Some(Attention::Silent)
    );
}

#[test]
fn set_then_cap_then_floor_compose_within_one_action() {
    let combined = rule(
        1,
        any(),
        RuleAction {
            set: Some(Attention::Human),
            cap: Some(Attention::Agent),
            floor: Some(Attention::Silent),
            ..RuleAction::default()
        },
    );
    assert_eq!(
        Case::new("x.y")
            .evaluate(&[combined], Attention::Silent)
            .attention,
        Some(Attention::Agent)
    );
    let lifted = rule(
        1,
        any(),
        RuleAction {
            cap: Some(Attention::Agent),
            floor: Some(Attention::Agent),
            ..RuleAction::default()
        },
    );
    assert_eq!(
        Case::new("x.y")
            .evaluate(&[lifted], Attention::Silent)
            .attention,
        Some(Attention::Agent)
    );
}

#[test]
fn cap_and_floor_across_rules_compose() {
    let case = Case::new("x.y");
    let rules = [
        rule(1, any(), cap(Attention::Silent)),
        rule(2, any(), floor(Attention::Agent)),
    ];
    assert_eq!(
        case.evaluate(&rules, Attention::Human).attention,
        Some(Attention::Agent)
    );
}

#[test]
fn mute_withholds_and_stops_later_rules() {
    let muted = rule(1, any(), mute());
    let later = rule(2, any(), set(Attention::Human));
    let outcome = Case::new("x.y").evaluate(&[muted.clone(), later], Attention::Agent);
    assert_eq!(outcome.attention, None);
    assert_eq!(outcome.matched, vec![muted.id.clone()]);
    assert_eq!(outcome.changed_by, Some(muted.id));
}

#[test]
fn stream_resource_and_agent_predicates_must_all_hold() {
    let mut case = Case::new("stream.published");
    case.event.stream = Some(stream_id("news"));
    case.event.subjects = vec![SubjectRef::new("document", "d1")];
    case.event.actor_is_agent = true;
    let all = RuleMatch {
        streams: vec![stream_id("news")],
        resources: vec![SubjectRef::new("document", "d1")],
        actor_is_agent: Some(true),
        ..RuleMatch::default()
    };
    assert_eq!(
        case.evaluate(
            &[rule(1, all.clone(), set(Attention::Silent))],
            Attention::Agent
        )
        .attention,
        Some(Attention::Silent)
    );
    let human_only = RuleMatch {
        actor_is_agent: Some(false),
        ..all.clone()
    };
    assert_eq!(
        case.evaluate(
            &[rule(1, human_only, set(Attention::Silent))],
            Attention::Agent
        )
        .attention,
        Some(Attention::Agent)
    );
    let other_stream = RuleMatch {
        streams: vec![stream_id("other")],
        ..all
    };
    assert_eq!(
        case.evaluate(
            &[rule(1, other_stream, set(Attention::Silent))],
            Attention::Agent
        )
        .attention,
        Some(Attention::Agent)
    );
}

#[test]
fn recipient_and_actor_subjects_match_from_resolved_memberships() {
    let mut case = Case::new("x.y");
    case.recipient_subjects = vec![role("sales")];
    case.actor_subjects = vec![role("bots")];
    let by_role = rule(1, recipients(&[role("sales")]), set(Attention::Human));
    assert_eq!(
        case.evaluate(&[by_role], Attention::Silent).attention,
        Some(Attention::Human)
    );
    let by_user = rule(
        1,
        recipients(&[AudienceSubject::User(UserId::new("reader"))]),
        set(Attention::Human),
    );
    assert_eq!(
        case.evaluate(&[by_user], Attention::Silent).attention,
        Some(Attention::Human)
    );
    let not_member = rule(1, recipients(&[role("support")]), set(Attention::Human));
    assert_eq!(
        case.evaluate(&[not_member], Attention::Silent).attention,
        Some(Attention::Silent)
    );
    let by_actor = rule(
        1,
        RuleMatch {
            actors: vec![role("bots")],
            ..RuleMatch::default()
        },
        mute(),
    );
    assert_eq!(case.evaluate(&[by_actor], Attention::Agent).attention, None);
}

#[test]
fn an_unresolved_subject_can_lower_attention_but_never_raise_it() {
    let mut case = Case::new("x.y");
    case.unresolved = vec![role("sales")];
    let raising = rule(1, recipients(&[role("sales")]), floor(Attention::Human));
    let outcome = case.evaluate(&[raising], Attention::Silent);
    assert_eq!(outcome.attention, Some(Attention::Silent));
    assert!(outcome.matched.is_empty());

    let lowering = rule(1, recipients(&[role("sales")]), cap(Attention::Silent));
    assert_eq!(
        case.evaluate(&[lowering], Attention::Human).attention,
        Some(Attention::Silent)
    );
    let muting = rule(1, recipients(&[role("sales")]), mute());
    assert_eq!(case.evaluate(&[muting], Attention::Human).attention, None);
}

#[test]
fn coalescing_and_digest_come_only_from_rules_that_definitely_match() {
    let mut case = Case::new("x.y");
    case.unresolved = vec![role("sales")];
    let definite = rule(
        1,
        any(),
        RuleAction {
            coalesce_window_secs: Some(300),
            digest: Some(DigestWindow { every_secs: 3600 }),
            ..RuleAction::default()
        },
    );
    let uncertain = rule(
        2,
        recipients(&[role("sales")]),
        RuleAction {
            coalesce_window_secs: Some(60),
            digest: Some(DigestWindow { every_secs: 60 }),
            ..RuleAction::default()
        },
    );
    let outcome = case.evaluate(&[definite, uncertain], Attention::Agent);
    assert_eq!(outcome.coalesce_window_secs, Some(300));
    assert_eq!(outcome.digest, Some(DigestWindow { every_secs: 3600 }));
}

#[test]
fn global_rules_then_the_recipients_own_rules() {
    let global = rule(-100, any(), cap(Attention::Agent));
    let own = rule(-1000, any(), floor(Attention::Human));
    let outcome = Case::new("x.y").evaluate(&[global, own.clone()], Attention::Silent);
    assert_eq!(
        outcome.attention,
        Some(Attention::Human),
        "the recipient's own rule runs last whatever its priority"
    );
    assert_eq!(outcome.changed_by, Some(own.id));
}

fn quiet(tz: &str) -> RuleAction {
    RuleAction {
        quiet_hours: Some(QuietHours {
            start: NaiveTime::from_hms_opt(22, 0, 0).expect("time"),
            end: NaiveTime::from_hms_opt(7, 0, 0).expect("time"),
            tz: tz.to_owned(),
            weekdays: None,
        }),
        ..RuleAction::default()
    }
}

#[test]
fn a_rule_inside_its_quiet_hours_reports_quiet_and_leaves_the_attention_to_the_last_step() {
    let mut case = Case::new("x.y");
    case.now = utc(2026, 10, 29, 23, 30);
    let outcome = case.evaluate(&[rule(1, any(), quiet("UTC"))], Attention::Human);
    assert!(outcome.quiet);
    assert_eq!(
        outcome.attention,
        Some(Attention::Human),
        "quiet hours apply after the subscription ceiling, outside the rules"
    );

    case.now = utc(2026, 10, 29, 12, 0);
    let outcome = case.evaluate(&[rule(1, any(), quiet("UTC"))], Attention::Human);
    assert!(!outcome.quiet);
}

#[test]
fn quiet_hours_honour_a_fixed_offset() {
    let mut case = Case::new("x.y");
    case.now = utc(2026, 10, 29, 21, 30);
    assert!(
        case.evaluate(&[rule(1, any(), quiet("+01:00"))], Attention::Human)
            .quiet,
        "21:30 UTC is 22:30 at +01:00"
    );
    assert!(
        !case
            .evaluate(&[rule(1, any(), quiet("UTC"))], Attention::Human)
            .quiet,
        "21:30 UTC is outside a UTC window"
    );
    case.now = utc(2026, 10, 29, 6, 30);
    assert!(
        !case
            .evaluate(&[rule(1, any(), quiet("+01:00"))], Attention::Human)
            .quiet,
        "06:30 UTC is 07:30 at +01:00, past the window"
    );
}

#[test]
fn an_unknown_time_zone_is_treated_as_inside_the_window() {
    let mut case = Case::new("x.y");
    case.now = utc(2026, 10, 29, 12, 0);
    let outcome = case.evaluate(&[rule(1, any(), quiet("Europe/London"))], Attention::Human);
    assert!(outcome.quiet);
}

#[test]
fn quiet_hours_on_a_rule_that_does_not_match_are_not_reported() {
    let mut case = Case::new("x.y");
    case.now = utc(2026, 10, 29, 23, 0);
    let elsewhere = rule(1, kinds(&["deal.*"]), quiet("UTC"));
    assert!(!case.evaluate(&[elsewhere], Attention::Human).quiet);
}

fn banded(floor: Option<Attention>, ceiling: Option<Attention>) -> StreamDecl {
    stream(
        "banded",
        StreamAttention {
            default: Attention::Agent,
            floor,
            ceiling,
        },
    )
}

#[test]
fn a_stream_floor_raises_and_takes_the_reason() {
    let decl = banded(Some(Attention::Agent), None);
    let (attention, reason) =
        RuleEvaluator::stream_bands(Attention::Silent, AttentionReason::Default, Some(&decl));
    assert_eq!(attention, Attention::Agent);
    assert_eq!(reason, AttentionReason::StreamFloor(stream_id("banded")));
}

#[test]
fn a_stream_ceiling_lowers_and_takes_the_reason() {
    let decl = banded(None, Some(Attention::Agent));
    let (attention, reason) =
        RuleEvaluator::stream_bands(Attention::Human, AttentionReason::Sender, Some(&decl));
    assert_eq!(attention, Attention::Agent);
    assert_eq!(reason, AttentionReason::StreamCeiling(stream_id("banded")));
}

#[test]
fn inside_the_band_the_reason_is_kept() {
    let decl = banded(Some(Attention::Silent), Some(Attention::Human));
    let (attention, reason) =
        RuleEvaluator::stream_bands(Attention::Agent, AttentionReason::Sender, Some(&decl));
    assert_eq!(attention, Attention::Agent);
    assert_eq!(reason, AttentionReason::Sender);
}

#[test]
fn without_a_stream_the_bands_change_nothing() {
    let (attention, reason) =
        RuleEvaluator::stream_bands(Attention::Human, AttentionReason::Sender, None);
    assert_eq!(attention, Attention::Human);
    assert_eq!(reason, AttentionReason::Sender);
}

#[test]
fn a_rule_cap_after_the_stream_floor_wins_because_bands_apply_first() {
    let decl = banded(Some(Attention::Agent), None);
    let (banded_attention, _) =
        RuleEvaluator::stream_bands(Attention::Silent, AttentionReason::Default, Some(&decl));
    let capped = rule(1, any(), cap(Attention::Silent));
    let outcome = Case::new("x.y").evaluate(std::slice::from_ref(&capped), banded_attention);
    assert_eq!(outcome.attention, Some(Attention::Silent));
    assert_eq!(outcome.changed_by, Some(capped.id));
}
