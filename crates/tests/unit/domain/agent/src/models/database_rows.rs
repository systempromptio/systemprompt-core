use chrono::Utc;
use systemprompt_agent::models::database_rows::{UserContextRow, UserContextWithStatsRow};
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::{UserContext, UserContextWithStats};

#[test]
fn user_context_row_from_converts_fields() {
    let now = Utc::now();
    let row = UserContextRow {
        context_id: ContextId::new("11111111-1111-4111-8111-111111111111"),
        user_id: UserId::new("alice"),
        name: "main".to_string(),
        created_at: now,
        updated_at: now,
    };
    let ctx: UserContext = row.into();
    assert_eq!(ctx.user_id.as_str(), "alice");
    assert_eq!(ctx.name, "main");
    assert_eq!(ctx.created_at, now);
}

#[test]
fn user_context_row_clone_debug() {
    let now = Utc::now();
    let row = UserContextRow {
        context_id: ContextId::new("22222222-2222-4222-8222-222222222222"),
        user_id: UserId::new("u1"),
        name: "ctx-1".to_string(),
        created_at: now,
        updated_at: now,
    };
    let cloned = row.clone();
    assert_eq!(cloned.user_id.as_str(), row.user_id.as_str());
    let dbg = format!("{:?}", row);
    assert!(dbg.contains("UserContextRow"));
}

#[test]
fn user_context_row_serde_roundtrip() {
    let now = Utc::now();
    let row = UserContextRow {
        context_id: ContextId::new("33333333-3333-4333-8333-333333333333"),
        user_id: UserId::new("ser-u"),
        name: "ser-ctx".to_string(),
        created_at: now,
        updated_at: now,
    };
    let json = serde_json::to_string(&row).expect("serialize");
    let de: UserContextRow = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de.user_id.as_str(), "ser-u");
    assert_eq!(de.name, "ser-ctx");
}

#[test]
fn user_context_with_stats_row_from_converts_fields() {
    let now = Utc::now();
    let row = UserContextWithStatsRow {
        context_id: ContextId::new("44444444-4444-4444-8444-444444444444"),
        user_id: UserId::new("stats-u"),
        name: "ctx-stats".to_string(),
        created_at: now,
        updated_at: now,
        task_count: 7,
        message_count: 42,
        last_message_at: Some(now),
    };
    let ctx: UserContextWithStats = row.into();
    assert_eq!(ctx.task_count, 7);
    assert_eq!(ctx.message_count, 42);
    assert_eq!(ctx.last_message_at, Some(now));
}

#[test]
fn user_context_with_stats_row_none_last_message() {
    let now = Utc::now();
    let row = UserContextWithStatsRow {
        context_id: ContextId::new("55555555-5555-4555-8555-555555555555"),
        user_id: UserId::new("u"),
        name: "n".to_string(),
        created_at: now,
        updated_at: now,
        task_count: 0,
        message_count: 0,
        last_message_at: None,
    };
    let ctx: UserContextWithStats = row.into();
    assert!(ctx.last_message_at.is_none());
}
