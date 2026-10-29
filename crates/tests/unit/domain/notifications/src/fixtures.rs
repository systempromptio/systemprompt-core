use chrono::{DateTime, TimeZone, Utc};
use systemprompt_identifiers::{NotificationRuleId, NotificationStreamId, UserId};
use systemprompt_models::notifications::{
    Attention, AudienceSubject, NotificationEvent, NotificationKind, RuleAction, RuleDecl,
    RuleMatch, StreamAttention, StreamDecl,
};

pub(crate) fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).single().expect("timestamp")
}

pub(crate) fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0)
        .single()
        .expect("timestamp")
}

pub(crate) fn kind(value: &str) -> NotificationKind {
    NotificationKind::new(value).expect("kind")
}

pub(crate) fn event(value: &str) -> NotificationEvent {
    NotificationEvent::new(kind(value), UserId::new("actor"), at(1_000_000))
}

pub(crate) fn stream_id(value: &str) -> NotificationStreamId {
    NotificationStreamId::try_new(value).expect("stream id")
}

pub(crate) fn rule(priority: i32, matcher: RuleMatch, action: RuleAction) -> RuleDecl {
    RuleDecl {
        id: NotificationRuleId::generate(),
        name: format!("rule {priority}"),
        priority,
        r#match: matcher,
        action,
    }
}

pub(crate) fn any() -> RuleMatch {
    RuleMatch::default()
}

pub(crate) fn set(level: Attention) -> RuleAction {
    RuleAction {
        set: Some(level),
        ..RuleAction::default()
    }
}

pub(crate) fn cap(level: Attention) -> RuleAction {
    RuleAction {
        cap: Some(level),
        ..RuleAction::default()
    }
}

pub(crate) fn floor(level: Attention) -> RuleAction {
    RuleAction {
        floor: Some(level),
        ..RuleAction::default()
    }
}

pub(crate) fn mute() -> RuleAction {
    RuleAction {
        mute: true,
        ..RuleAction::default()
    }
}

pub(crate) fn stream(id: &str, attention: StreamAttention) -> StreamDecl {
    StreamDecl {
        id: stream_id(id),
        name: id.to_owned(),
        description: None,
        audience: vec![AudienceSubject::Everyone],
        publishers: vec![AudienceSubject::Everyone],
        attention,
        retention_days: None,
        coalesce_window_secs: None,
    }
}
