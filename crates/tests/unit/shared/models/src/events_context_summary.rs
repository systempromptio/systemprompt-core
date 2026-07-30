//! Tests for the owned and borrowed `UserContextWithStats -> ContextSummary`
//! conversions.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{TimeZone, Utc};
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::api::contexts::{ContextKind, UserContextWithStats};
use systemprompt_models::events::payloads::ContextSummary;

fn sample() -> UserContextWithStats {
    UserContextWithStats {
        context_id: ContextId::generate(),
        user_id: UserId::new("user-7"),
        name: "Research thread".to_owned(),
        kind: ContextKind::User,
        created_at: Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2026, 3, 2, 11, 45, 0).unwrap(),
        task_count: 4,
        message_count: 17,
        last_message_at: Some(Utc.with_ymd_and_hms(2026, 3, 2, 11, 44, 0).unwrap()),
    }
}

fn assert_matches_sample(summary: &ContextSummary, source: &UserContextWithStats) {
    assert_eq!(summary.context_id, source.context_id);
    assert_eq!(summary.name, source.name);
    assert_eq!(summary.created_at, source.created_at);
    assert_eq!(summary.updated_at, source.updated_at);
    assert_eq!(summary.message_count, source.message_count);
    assert_eq!(summary.task_count, source.task_count);
}

#[test]
fn owned_conversion_carries_every_field() {
    let source = sample();
    let summary = ContextSummary::from(source.clone());
    assert_matches_sample(&summary, &source);
}

#[test]
fn borrowed_conversion_carries_every_field_and_leaves_source_intact() {
    let source = sample();
    let summary = ContextSummary::from(&source);

    assert_matches_sample(&summary, &source);
    assert!(
        !source.context_id.as_str().is_empty(),
        "borrowed conversion must not move out of the source"
    );
    assert_eq!(source.name, "Research thread");
}

#[test]
fn owned_and_borrowed_conversions_agree() {
    let source = sample();
    let owned = ContextSummary::from(source.clone());
    let borrowed = ContextSummary::from(&source);

    assert_eq!(
        serde_json::to_value(&owned).unwrap(),
        serde_json::to_value(&borrowed).unwrap()
    );
}

#[test]
fn conversion_drops_fields_absent_from_the_summary() {
    let source = sample();
    let expected_id = source.context_id.clone();
    let value = serde_json::to_value(ContextSummary::from(source)).unwrap();
    let obj = value.as_object().unwrap();

    assert!(!obj.contains_key("user_id"));
    assert!(!obj.contains_key("kind"));
    assert!(!obj.contains_key("last_message_at"));
    assert_eq!(obj["context_id"], serde_json::json!(expected_id.as_str()));
}

#[test]
fn zero_counts_are_preserved_not_defaulted() {
    let mut source = sample();
    source.message_count = 0;
    source.task_count = 0;

    let summary = ContextSummary::from(source);
    assert_eq!(summary.message_count, 0);
    assert_eq!(summary.task_count, 0);
}

#[test]
fn empty_name_survives_conversion() {
    let mut source = sample();
    source.name = String::new();

    assert_eq!(ContextSummary::from(&source).name, "");
    assert_eq!(ContextSummary::from(source).name, "");
}
