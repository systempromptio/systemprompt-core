use chrono::{NaiveTime, Weekday};
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId};
use systemprompt_models::notifications::{
    Attention, AudienceSubject, DeclError, DigestWindow, NotificationsDoc, SubscriptionTarget,
    validate,
};

const RULE_A: &str = "0d7f4e1c-1a2b-4c3d-8e9f-001122334455";
const RULE_B: &str = "6a1e2f3b-4c5d-4e6f-9a0b-66778899aabb";

const STREAMS: &str = r"
streams:
  - id: sales-news
    name: Sales news
    description: Announcements for the sales team
    audience: [role:sales, group:emea]
    publishers: [role:sales_lead]
    attention:
      default: agent
      floor: silent
      ceiling: human
    retention_days: 30
    coalesce_window_secs: 300
  - id: ops
    name: Operations
    audience: [everyone]
    publishers: [role:admin]
";

fn rules_yaml() -> String {
    format!(
        r"
rules:
  - id: {RULE_A}
    name: Agents never interrupt
    priority: 10
    match:
      kinds: ['document.*']
      actor_is_agent: true
    action:
      cap: agent
  - id: {RULE_B}
    name: Quiet weekends on sales news
    priority: 20
    match:
      streams: [sales-news]
      recipients: [role:sales]
    action:
      digest: {{every_secs: 3600}}
      quiet_hours:
        start: '22:00'
        end: '07:00'
        tz: '+01:00'
        weekdays: [Sat, Sun]
"
    )
}

const SUBSCRIPTIONS: &str = r"
subscriptions:
  - subject: role:support
    target: {stream: ops}
    attention_ceiling: agent
  - subject: group:sales
    target:
      resource: {kind: document, id: 3f1c2b4a-0000-4000-8000-000000000001}
    digest: {every_secs: 900}
";

fn parse(yaml: &str) -> NotificationsDoc {
    serde_yaml::from_str(yaml).expect("declaration parses")
}

fn realistic() -> NotificationsDoc {
    let mut doc = parse(STREAMS);
    doc.merge(parse(&rules_yaml()));
    doc.merge(parse(SUBSCRIPTIONS));
    doc
}

fn stream(id: &str) -> NotificationStreamId {
    NotificationStreamId::try_new(id).expect("stream id")
}

#[test]
fn three_files_merge_into_one_valid_declaration_set() {
    let doc = realistic();
    assert_eq!(validate(&doc), Ok(()));
    assert_eq!(doc.streams.len(), 2);
    assert_eq!(doc.rules.len(), 2);
    assert_eq!(doc.subscriptions.len(), 2);

    let sales = &doc.streams[0];
    assert_eq!(sales.id, stream("sales-news"));
    assert_eq!(
        sales.audience,
        vec![
            AudienceSubject::Role("sales".to_owned()),
            AudienceSubject::Dimension {
                kind: "group".to_owned(),
                value: "emea".to_owned()
            },
        ]
    );
    assert_eq!(sales.attention.default, Attention::Agent);
    assert_eq!(sales.attention.floor, Some(Attention::Silent));
    assert_eq!(sales.attention.ceiling, Some(Attention::Human));
    assert_eq!(sales.retention_days, Some(30));
    assert_eq!(sales.coalesce_window_secs, Some(300));

    let ops = &doc.streams[1];
    assert_eq!(ops.audience, vec![AudienceSubject::Everyone]);
    assert_eq!(
        ops.attention.default,
        Attention::Silent,
        "default attention is silent"
    );
    assert_eq!(ops.attention.floor, None);
}

#[test]
fn rules_parse_their_match_and_action() {
    let doc = realistic();
    let cap = &doc.rules[0];
    assert_eq!(
        cap.id,
        NotificationRuleId::try_new(RULE_A).expect("rule id")
    );
    assert_eq!(cap.priority, 10);
    assert_eq!(cap.r#match.kinds, vec!["document.*".to_owned()]);
    assert_eq!(cap.r#match.actor_is_agent, Some(true));
    assert_eq!(cap.action.cap, Some(Attention::Agent));
    assert!(!cap.action.mute);

    let quiet = &doc.rules[1];
    assert_eq!(quiet.r#match.streams, vec![stream("sales-news")]);
    assert_eq!(quiet.action.digest, Some(DigestWindow { every_secs: 3600 }));
    let hours = quiet.action.quiet_hours.as_ref().expect("quiet hours");
    assert_eq!(
        hours.start,
        NaiveTime::from_hms_opt(22, 0, 0).expect("time")
    );
    assert_eq!(hours.end, NaiveTime::from_hms_opt(7, 0, 0).expect("time"));
    assert_eq!(hours.tz, "+01:00");
    assert_eq!(hours.weekdays, Some(vec![Weekday::Sat, Weekday::Sun]));
}

#[test]
fn subscriptions_parse_stream_and_resource_targets() {
    let doc = realistic();
    assert_eq!(
        doc.subscriptions[0].target,
        SubscriptionTarget::Stream(stream("ops"))
    );
    assert_eq!(
        doc.subscriptions[0].attention_ceiling,
        Some(Attention::Agent)
    );
    match &doc.subscriptions[1].target {
        SubscriptionTarget::Resource(resource) => {
            assert_eq!(resource.kind, "document");
            assert_eq!(resource.id, "3f1c2b4a-0000-4000-8000-000000000001");
        },
        other => panic!("expected a resource target, got {other:?}"),
    }
    assert_eq!(
        doc.subscriptions[1].digest,
        Some(DigestWindow { every_secs: 900 })
    );
}

#[test]
fn unknown_fields_are_refused() {
    let typo = "streams:\n  - id: a\n    name: A\n    audience: [everyone]\n    publishers: [everyone]\n    retention: 3\n";
    assert!(serde_yaml::from_str::<NotificationsDoc>(typo).is_err());
    assert!(serde_yaml::from_str::<NotificationsDoc>("channels: []\n").is_err());
    let bad_stream_id = "streams:\n  - id: Sales News\n    name: A\n    audience: [everyone]\n    publishers: [everyone]\n";
    assert!(serde_yaml::from_str::<NotificationsDoc>(bad_stream_id).is_err());
}

#[test]
fn a_duplicate_stream_is_refused() {
    let mut doc = realistic();
    let again = doc.streams[1].clone();
    doc.streams.push(again);
    assert_eq!(
        validate(&doc),
        Err(DeclError::DuplicateStream(stream("ops")))
    );
}

#[test]
fn a_duplicate_rule_is_refused() {
    let mut doc = realistic();
    let again = doc.rules[0].clone();
    doc.rules.push(again);
    assert_eq!(
        validate(&doc),
        Err(DeclError::DuplicateRule(
            NotificationRuleId::try_new(RULE_A).expect("rule id")
        ))
    );
}

#[test]
fn a_stream_floor_above_its_ceiling_is_refused() {
    let mut doc = realistic();
    doc.streams[0].attention.floor = Some(Attention::Human);
    doc.streams[0].attention.ceiling = Some(Attention::Agent);
    assert_eq!(
        validate(&doc),
        Err(DeclError::StreamFloorAboveCeiling(stream("sales-news")))
    );
}

#[test]
fn a_rule_floor_above_its_cap_is_refused() {
    let mut doc = realistic();
    doc.rules[0].action.floor = Some(Attention::Human);
    assert_eq!(
        validate(&doc),
        Err(DeclError::RuleFloorAboveCap(
            NotificationRuleId::try_new(RULE_A).expect("rule id")
        ))
    );
}

#[test]
fn a_stream_without_audience_or_publishers_is_refused() {
    let mut doc = realistic();
    doc.streams[1].audience.clear();
    assert_eq!(validate(&doc), Err(DeclError::EmptyAudience(stream("ops"))));

    let mut doc = realistic();
    doc.streams[1].publishers.clear();
    assert_eq!(
        validate(&doc),
        Err(DeclError::EmptyPublishers(stream("ops")))
    );
}

#[test]
fn a_rule_naming_an_undeclared_stream_is_refused() {
    let mut doc = realistic();
    doc.rules[1].r#match.streams = vec![stream("ghost")];
    assert_eq!(
        validate(&doc),
        Err(DeclError::UnknownRuleStream {
            rule: NotificationRuleId::try_new(RULE_B).expect("rule id"),
            stream: stream("ghost"),
        })
    );
}

#[test]
fn a_subscription_to_an_undeclared_stream_is_refused() {
    let mut doc = realistic();
    doc.subscriptions[0].target = SubscriptionTarget::Stream(stream("ghost"));
    assert_eq!(
        validate(&doc),
        Err(DeclError::UnknownSubscriptionStream(stream("ghost")))
    );
}
