use systemprompt_events::{OUTBOX_CHANNEL, OutboxChannel};

#[test]
fn outbox_channel_constant_value() {
    assert_eq!(OUTBOX_CHANNEL, "systemprompt_events");
}

#[test]
fn agui_as_str() {
    assert_eq!(OutboxChannel::AgUi.as_str(), "agui");
}

#[test]
fn a2a_as_str() {
    assert_eq!(OutboxChannel::A2A.as_str(), "a2a");
}

#[test]
fn system_as_str() {
    assert_eq!(OutboxChannel::System.as_str(), "system");
}

#[test]
fn analytics_as_str() {
    assert_eq!(OutboxChannel::Analytics.as_str(), "analytics");
}

#[test]
fn parse_agui() {
    assert_eq!(OutboxChannel::parse("agui"), Some(OutboxChannel::AgUi));
}

#[test]
fn parse_a2a() {
    assert_eq!(OutboxChannel::parse("a2a"), Some(OutboxChannel::A2A));
}

#[test]
fn parse_system() {
    assert_eq!(OutboxChannel::parse("system"), Some(OutboxChannel::System));
}

#[test]
fn parse_analytics() {
    assert_eq!(
        OutboxChannel::parse("analytics"),
        Some(OutboxChannel::Analytics)
    );
}

#[test]
fn parse_unknown_returns_none() {
    assert_eq!(OutboxChannel::parse("unknown"), None);
}

#[test]
fn parse_empty_returns_none() {
    assert_eq!(OutboxChannel::parse(""), None);
}

#[test]
fn parse_case_sensitive() {
    assert_eq!(OutboxChannel::parse("AGUI"), None);
    assert_eq!(OutboxChannel::parse("A2A"), None);
    assert_eq!(OutboxChannel::parse("System"), None);
    assert_eq!(OutboxChannel::parse("Analytics"), None);
}

#[test]
fn round_trip_all_variants() {
    let variants = [
        OutboxChannel::AgUi,
        OutboxChannel::A2A,
        OutboxChannel::System,
        OutboxChannel::Analytics,
    ];
    for variant in variants {
        let s = variant.as_str();
        let parsed = OutboxChannel::parse(s);
        assert_eq!(parsed, Some(variant), "round-trip failed for {s}");
    }
}

#[test]
fn outbox_channel_debug() {
    let debug = format!("{:?}", OutboxChannel::AgUi);
    assert!(debug.contains("AgUi"));
}

#[test]
fn outbox_channel_clone_and_copy() {
    let ch = OutboxChannel::A2A;
    let ch2 = ch;
    assert_eq!(ch, ch2);
    let ch3 = ch2;
    assert_eq!(ch3, OutboxChannel::A2A);
}

#[test]
fn outbox_channel_partial_eq() {
    assert_eq!(OutboxChannel::System, OutboxChannel::System);
    assert_ne!(OutboxChannel::System, OutboxChannel::A2A);
}
