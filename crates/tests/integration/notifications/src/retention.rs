use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_notifications::NotificationRetentionJob;
use systemprompt_traits::{Dependencies, Job, JobContext};

use crate::harness::{Harness, profile};

async fn failure_aged(h: &Harness, reason: &str, days: i64) {
    sqlx::query(
        "INSERT INTO notification_delivery_failures (stage, reason, at) VALUES ('test', $1, $2)",
    )
    .bind(reason)
    .bind(Utc::now() - Duration::days(days))
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("failure row");
}

async fn unread_aged(h: &Harness, user: &UserId, key: &str, days: i64) {
    sqlx::query(
        "INSERT INTO notifications (recipient_user_id, kind, actor_user_id, actor_is_agent, \
         subjects, attention, attention_reason, action, coalesce_key, first_event_at, \
         last_event_at, created_at, outbox_id, delivery_state) \
         VALUES ($1, 'document.shared', $1, FALSE, '[]'::jsonb, 'silent', 'default', \
         '{}'::jsonb, $2, $3, $3, $3, 'aged', 'delivered')",
    )
    .bind(user.as_str())
    .bind(key)
    .bind(Utc::now() - Duration::days(days))
    .execute(h.pool.write_pool().as_ref())
    .await
    .expect("unread row");
}

async fn run(h: &Harness, owner: &UserId, profile: NotificationsProfile) {
    let ctx = JobContext::new(
        Actor::user(owner.clone()),
        Dependencies::new()
            .with(h.pool.clone())
            .with(Arc::new(profile)),
    );
    let result = NotificationRetentionJob
        .execute(&ctx)
        .await
        .expect("retention job");
    assert!(result.success);
}

async fn unread_keys(h: &Harness) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT coalesce_key FROM notifications WHERE read_at IS NULL ORDER BY coalesce_key",
    )
    .fetch_all(h.pool.write_pool().as_ref())
    .await
    .expect("unread")
}

async fn failure_reasons(h: &Harness) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT reason FROM notification_delivery_failures ORDER BY reason",
    )
    .fetch_all(h.pool.write_pool().as_ref())
    .await
    .expect("failures")
}

#[tokio::test]
async fn retention_purges_old_failures_and_unread_rows_past_their_profile_ages() {
    let h = Harness::new("notif_retention_purge").await;
    let reader = h.user("reader", &[]).await;
    let defaults = profile();
    failure_aged(
        &h,
        "expired",
        i64::from(defaults.failure_retention_days) + 1,
    )
    .await;
    failure_aged(&h, "recent", 1).await;
    unread_aged(
        &h,
        &reader,
        "expired",
        i64::from(defaults.unread_max_age_days) + 1,
    )
    .await;
    unread_aged(&h, &reader, "recent", 1).await;

    run(&h, &reader, defaults).await;

    assert_eq!(failure_reasons(&h).await, vec!["recent".to_owned()]);
    assert_eq!(unread_keys(&h).await, vec!["recent".to_owned()]);
    h.finish().await;
}

#[tokio::test]
async fn retention_keeps_what_the_profile_still_retains() {
    let h = Harness::new("notif_retention_keep").await;
    let reader = h.user("reader", &[]).await;
    failure_aged(&h, "aged", 40).await;
    unread_aged(&h, &reader, "aged", 200).await;

    run(
        &h,
        &reader,
        NotificationsProfile {
            failure_retention_days: 60,
            unread_max_age_days: 365,
            ..profile()
        },
    )
    .await;

    assert_eq!(failure_reasons(&h).await, vec!["aged".to_owned()]);
    assert_eq!(unread_keys(&h).await, vec!["aged".to_owned()]);
    h.finish().await;
}
