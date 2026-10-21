//! Failure settlement: a request that never produced a completion is
//! recorded as failed with whatever usage the provider streamed first, priced
//! like a completion, and a failure whose quota release cannot be written
//! still settles the request row.

use bytes::Bytes;
use systemprompt_gateway::GatewayAudit;
use systemprompt_gateway::protocol::canonical::CanonicalUsage;
use systemprompt_identifiers::AiRequestId;

use crate::quota_accounting::{
    dead_pool, gateway_repos_in, private_journal_dir, request_ctx, status_of,
};
use crate::support::{minimal_request, seed_user, setup_db};

async fn tokens_and_cost(
    db: &systemprompt_database::DbPool,
    id: &AiRequestId,
) -> (Option<i32>, Option<i32>, i64, Option<String>) {
    sqlx::query_as(
        "SELECT input_tokens, output_tokens, cost_microdollars, error_message \
         FROM ai_requests WHERE id = $1",
    )
    .bind(id.as_str())
    .fetch_one(db.pool().as_ref())
    .await
    .expect("request row")
}

async fn opened(
    repos: &systemprompt_gateway::GatewayRepositories,
    user_id: systemprompt_identifiers::UserId,
    id: &AiRequestId,
) -> GatewayAudit {
    let audit = GatewayAudit::new(repos, request_ctx(user_id, id.clone()));
    audit
        .open(
            &minimal_request(None, "will fail"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");
    audit
        .pin_pricing(systemprompt_manifest::services::ModelPricing {
            input_per_million: 1.0,
            output_per_million: 2.0,
            ..Default::default()
        })
        .expect("pin pricing");
    audit
}

#[tokio::test]
async fn a_truncated_stream_is_failed_and_priced_for_what_it_streamed() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let id = AiRequestId::generate();
    let repos = gateway_repos_in(&db, &private_journal_dir());
    let audit = opened(&repos, user_id, &id).await;

    audit
        .fail_with_usage(
            "upstream reset mid-stream",
            Some(CanonicalUsage {
                input_tokens: 11,
                output_tokens: 7,
                ..Default::default()
            }),
        )
        .await
        .expect("failure settles");

    assert_eq!(status_of(&db, &id).await, "failed");
    let (input, output, cost, error) = tokens_and_cost(&db, &id).await;
    assert_eq!((input, output, cost), (Some(11), Some(7), 25));
    assert_eq!(error.as_deref(), Some("upstream reset mid-stream"));
}

#[tokio::test]
async fn a_failure_with_no_billable_usage_records_no_tokens() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let id = AiRequestId::generate();
    let repos = gateway_repos_in(&db, &private_journal_dir());
    let audit = opened(&repos, user_id, &id).await;

    audit
        .fail_with_usage("refused before output", Some(CanonicalUsage::default()))
        .await
        .expect("failure settles");

    assert_eq!(status_of(&db, &id).await, "failed");
    let (_, _, cost, _) = tokens_and_cost(&db, &id).await;
    assert_eq!(cost, 0, "an empty usage is not billed");
}

#[tokio::test]
async fn a_failure_settles_even_when_the_quota_release_cannot_be_written() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let id = AiRequestId::generate();
    let mut repos = gateway_repos_in(&db, &private_journal_dir());
    repos.quota_buckets = systemprompt_ai::repository::AiQuotaBucketRepository::new(&dead_pool());
    let audit = opened(&repos, user_id.clone(), &id).await;
    audit.set_quota_reservation(crate::quota_accounting::reservation_for(&user_id));

    audit
        .fail("provider unavailable")
        .await
        .expect("failure settles");

    assert_eq!(status_of(&db, &id).await, "failed");
}

#[tokio::test]
async fn a_failure_before_admission_never_leaves_a_completed_row() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let id = AiRequestId::generate();
    let repos = gateway_repos_in(&db, &private_journal_dir());
    let audit = GatewayAudit::new(&repos, request_ctx(user_id, id.clone()));

    let _settled = audit.fail("rejected before open").await;

    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_requests WHERE id = $1 AND status = 'completed'",
    )
    .bind(id.as_str())
    .fetch_one(db.pool().as_ref())
    .await
    .expect("count");
    assert_eq!(completed, 0);
}
