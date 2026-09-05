//! `tokens_used` is aggregated as stored, and reasoning and cache counts are
//! reported beside it.
//!
//! The row inserted here deliberately gives `tokens_used` a value that no
//! re-summation of the component columns could produce, so a reader that
//! recomputes the total instead of aggregating the column fails loudly.

use chrono::{Duration, Utc};
use systemprompt_analytics::{CostAnalyticsRepository, RequestAnalyticsRepository};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::db_pool_or_skip;
use uuid::Uuid;

const TOKENS_USED: i32 = 9_999;
const INPUT: i32 = 100;
const OUTPUT: i32 = 40;
const REASONING: i32 = 25;
const CACHE_READ: i32 = 70;
const CACHE_CREATION: i32 = 30;

#[tokio::test]
async fn summary_and_stats_report_reasoning_and_keep_tokens_used_intact() {
    let (db, _url) = db_pool_or_skip!();
    let pool = db.pool_arc().expect("write pool");

    let nonce = Uuid::new_v4().simple().to_string();
    let user_id = UserId::new(format!("user-{nonce}"));
    let created_at = Utc::now();

    sqlx::query(
        "INSERT INTO ai_requests (
            id, request_id, user_id, context_id, provider, model,
            tokens_used, input_tokens, output_tokens,
            reasoning_tokens, cache_read_tokens, cache_creation_tokens,
            cost_microdollars, status, actor_kind, actor_id, synthetic, created_at
        ) VALUES ($1, $2, $3, $4, 'openai', 'gpt-test',
            $5, $6, $7, $8, $9, $10, 1234, 'completed', 'user', $3, FALSE, $11)",
    )
    .bind(format!("air-{nonce}"))
    .bind(format!("req-{nonce}"))
    .bind(user_id.as_str())
    .bind(format!("ctx-{nonce}"))
    .bind(TOKENS_USED)
    .bind(INPUT)
    .bind(OUTPUT)
    .bind(REASONING)
    .bind(CACHE_READ)
    .bind(CACHE_CREATION)
    .bind(created_at)
    .execute(&*pool)
    .await
    .expect("insert the audited request");

    let start = created_at - Duration::seconds(1);
    let end = created_at + Duration::seconds(1);

    let costs = CostAnalyticsRepository::new(&db).expect("cost repository");
    let summary = costs
        .get_summary_for_user(&user_id, start, end)
        .await
        .expect("per-user cost summary");

    assert_eq!(summary.requests, 1);
    assert_eq!(
        summary.tokens,
        Some(i64::from(TOKENS_USED)),
        "tokens_used is aggregated as stored, never re-summed from components"
    );
    assert_eq!(summary.reasoning_tokens, Some(i64::from(REASONING)));
    assert_eq!(summary.cache_read_tokens, Some(i64::from(CACHE_READ)));
    assert_eq!(
        summary.cache_creation_tokens,
        Some(i64::from(CACHE_CREATION))
    );

    let requests = RequestAnalyticsRepository::new(&db).expect("request repository");
    let stats = requests
        .get_stats(start, end, Some("gpt-test"))
        .await
        .expect("request stats");

    assert_eq!(stats.total, 1);
    assert_eq!(stats.total_tokens, Some(i64::from(TOKENS_USED)));
    assert_eq!(stats.reasoning_tokens, Some(i64::from(REASONING)));
    assert_eq!(stats.cache_read_tokens, Some(i64::from(CACHE_READ)));
    assert_eq!(stats.cache_creation_tokens, Some(i64::from(CACHE_CREATION)));

    sqlx::query("DELETE FROM ai_requests WHERE id = $1")
        .bind(format!("air-{nonce}"))
        .execute(&*pool)
        .await
        .expect("clean up the audited request");
}
