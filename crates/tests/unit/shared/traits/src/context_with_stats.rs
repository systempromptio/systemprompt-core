use chrono::{Duration, Utc};
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_traits::{ContextStats, ContextWithStats};

#[test]
fn a_new_context_starts_with_zero_stats() {
    let created = Utc::now();
    let updated = created + Duration::seconds(5);
    let context_id = ContextId::generate();
    let ctx = ContextWithStats::new(
        context_id.clone(),
        UserId::new("user-stats-new"),
        "Planning",
        created,
        updated,
    );
    assert_eq!(ctx.context_id, context_id);
    assert_eq!(ctx.user_id.as_str(), "user-stats-new");
    assert_eq!(ctx.name, "Planning");
    assert_eq!(ctx.created_at, created);
    assert_eq!(ctx.updated_at, updated);
    assert_eq!(ctx.task_count, 0);
    assert_eq!(ctx.message_count, 0);
    assert!(ctx.last_message_at.is_none());
}

#[test]
fn with_stats_overwrites_every_counter() {
    let now = Utc::now();
    let last = now - Duration::minutes(3);
    let ctx = ContextWithStats::new(
        ContextId::generate(),
        UserId::new("user-stats-apply"),
        String::from("Review"),
        now,
        now,
    )
    .with_stats(ContextStats {
        task_count: 4,
        message_count: 17,
        last_message_at: Some(last),
    });
    assert_eq!(ctx.task_count, 4);
    assert_eq!(ctx.message_count, 17);
    assert_eq!(ctx.last_message_at, Some(last));

    let reset = ctx.with_stats(ContextStats::default());
    assert_eq!(reset.task_count, 0);
    assert_eq!(reset.message_count, 0);
    assert!(reset.last_message_at.is_none());
    assert_eq!(reset.name, "Review");
}
