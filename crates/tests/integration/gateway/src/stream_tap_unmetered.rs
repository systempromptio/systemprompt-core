//! A stream that completes with content but reports no usage is settled to the
//! larger of its reservation and what was observed: it is neither recorded as
//! free nor refunded.

use systemprompt_gateway::policies::GatewayPolicySpec;
use systemprompt_gateway::protocol::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::quota::{AccountingOutcome, CostFloor, QuotaUsage, settle_unmetered};
use systemprompt_gateway::stream_tap::retained::MAX_RETAINED_STREAM_BYTES;
use systemprompt_gateway::stream_tap::tap;
use systemprompt_manifest::services::QuotaFaultMode;

use crate::quota_reserve::{admitted, bucket, repos, reserve, window};
use crate::stream_tap_pipeline::{
    admit, events_stream, open_audit, quota_bucket, render, settled_status, tap_ctx, user_window,
};
use crate::support::{seed_user, setup_db};

pub(crate) async fn usage_source(
    db: &systemprompt_database::DbPool,
    id: &systemprompt_identifiers::AiRequestId,
) -> String {
    sqlx::query_scalar("SELECT usage_source FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(db.pool().as_ref())
        .await
        .expect("usage_source")
}

#[tokio::test]
async fn a_completed_stream_without_usage_keeps_its_reservation() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id.clone()).await;
    let policy = GatewayPolicySpec {
        quota_windows: vec![user_window(3600)],
        ..GatewayPolicySpec::default()
    };
    admit(&audit, &db, &policy).await;

    let upstream = events_stream(vec![
        Ok(CanonicalEvent::MessageStart {
            id: "resp-unmetered".to_owned(),
            model: "claude-served".to_owned(),
            usage: CanonicalUsage::default(),
        }),
        Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }),
        Ok(CanonicalEvent::TextDelta {
            index: 0,
            text: "an answer the upstream never metered".to_owned(),
        }),
        Ok(CanonicalEvent::ContentBlockStop { index: 0 }),
        Ok(CanonicalEvent::MessageStop {
            id: "resp-unmetered".to_owned(),
            stop_reason: Some(CanonicalStopReason::EndTurn),
            raw_finish_reason: None,
        }),
    ]);
    let body = tap(
        upstream,
        render(std::sync::Arc::new(AnthropicMessagesInbound)),
        std::sync::Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, policy),
    );
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("tapped body");
    assert!(
        String::from_utf8_lossy(&bytes).contains("never metered"),
        "the client still receives the stream"
    );

    let (status, _) = settled_status(&audit, &db, &ai_request_id).await;
    assert_eq!(
        status, "completed",
        "an unmetered stream is still a success"
    );
    assert_eq!(usage_source(&db, &ai_request_id).await, "reservation");
    assert_eq!(
        quota_bucket(&db, &user_id).await,
        Some((50, 60, 70)),
        "an unmetered stream settles to its reservation, not to zero"
    );
}

#[tokio::test]
async fn unmetered_settlement_keeps_the_larger_of_reservation_and_observation() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let windows = [window("user", 1260, None)];
    let reservation = admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let outcome = settle_unmetered(
        &repos.quota_buckets,
        &reservation,
        QuotaUsage {
            input_tokens: 10,
            output_tokens: 500,
            cost_microdollars: 5,
        },
        CostFloor::Reserved,
    )
    .await;
    assert!(matches!(outcome, AccountingOutcome::Counted), "{outcome:?}");
    assert_eq!(bucket(&db, &user, 1260).await, (1, 50, 500, 70));
}

#[tokio::test]
async fn unmetered_settlement_billed_to_a_user_key_charges_no_cost() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let windows = [window("user", 1290, None)];
    let reservation = admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let outcome = settle_unmetered(
        &repos.quota_buckets,
        &reservation,
        QuotaUsage::default(),
        CostFloor::Waived,
    )
    .await;
    assert!(matches!(outcome, AccountingOutcome::Counted), "{outcome:?}");
    assert_eq!(bucket(&db, &user, 1290).await, (1, 50, 60, 0));
}

#[tokio::test]
async fn a_stream_longer_than_the_retention_records_a_truncated_payload() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id.clone()).await;
    let mut events = vec![
        Ok(CanonicalEvent::MessageStart {
            id: "resp-long".to_owned(),
            model: "claude-served".to_owned(),
            usage: CanonicalUsage {
                input_tokens: 3,
                ..CanonicalUsage::default()
            },
        }),
        Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }),
    ];
    for _ in 0..10 {
        events.push(Ok(CanonicalEvent::TextDelta {
            index: 0,
            text: "w".repeat(1024 * 1024),
        }));
    }
    events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
    events.push(Ok(CanonicalEvent::MessageStop {
        id: "resp-long".to_owned(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
        raw_finish_reason: None,
    }));
    let body = tap(
        events_stream(events),
        render(std::sync::Arc::new(AnthropicMessagesInbound)),
        std::sync::Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let delivered = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("tapped body")
        .len();
    assert!(delivered > MAX_RETAINED_STREAM_BYTES, "{delivered}");

    let (status, _) = settled_status(&audit, &db, &ai_request_id).await;
    assert_eq!(status, "completed");
    let (truncated, bytes, excerpt): (bool, Option<i32>, Option<String>) = sqlx::query_as(
        "SELECT response_truncated, response_bytes, response_excerpt \
         FROM ai_request_payloads WHERE ai_request_id = $1",
    )
    .bind(ai_request_id.as_str())
    .fetch_one(db.pool().as_ref())
    .await
    .expect("payload row");
    assert!(truncated, "the stored response is a prefix");
    assert_eq!(
        bytes.map(i64::from),
        Some(i64::try_from(delivered).expect("fits")),
        "response_bytes is the full streamed length"
    );
    let excerpt = excerpt.expect("excerpt");
    assert!(
        excerpt.contains(&format!(
            "<retained {MAX_RETAINED_STREAM_BYTES} of {delivered} bytes; sha256 covers the retained prefix only>"
        )),
        "{}",
        &excerpt[..excerpt.len().min(200)]
    );
}
