//! Quota exits that the happy paths do not reach: a failure whose audit write
//! fails still releases the request's reservation.

use systemprompt_manifest::services::QuotaFaultMode;

use crate::quota_reserve::{admitted, bucket, repos, reserve, window};
use crate::stream_tap_pipeline::open_audit;
use crate::support::{seed_user, setup_db};

#[tokio::test]
async fn a_failure_whose_audit_write_fails_still_releases_the_reservation() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id.clone()).await;
    let repos = repos(&db);
    let reservation = admitted(
        reserve(
            &repos,
            &user_id,
            &[window("user", 1320, None)],
            QuotaFaultMode::Closed,
        )
        .await,
    );
    assert_eq!(bucket(&db, &user_id, 1320).await, (1, 50, 60, 70));
    audit.set_quota_reservation(reservation);

    sqlx::query("DELETE FROM ai_requests WHERE id = $1")
        .bind(ai_request_id.as_str())
        .execute(db.pool().as_ref())
        .await
        .expect("remove the audit row so its settlement cannot land");

    let outcome = audit.fail("upstream request failed").await;
    assert!(
        outcome.is_err(),
        "the audit write cannot land on a missing row"
    );
    assert_eq!(
        bucket(&db, &user_id, 1320).await,
        (1, 0, 0, 0),
        "the reservation is released even though the audit write failed"
    );
}

fn opened_stream(
    id: &str,
    input_tokens: u32,
) -> futures::stream::BoxStream<
    'static,
    Result<
        systemprompt_gateway::protocol::canonical::CanonicalEvent,
        systemprompt_wire::error::WireStreamError,
    >,
> {
    use futures::StreamExt;
    use systemprompt_gateway::protocol::canonical::{
        CanonicalEvent, CanonicalUsage, ContentBlockKind,
    };
    futures::stream::iter(vec![
        Ok(CanonicalEvent::MessageStart {
            id: id.to_owned(),
            model: "claude-served".to_owned(),
            usage: CanonicalUsage {
                input_tokens,
                ..CanonicalUsage::default()
            },
        }),
        Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }),
        Ok(CanonicalEvent::TextDelta {
            index: 0,
            text: "partial".to_owned(),
        }),
    ])
    .chain(futures::stream::pending())
    .boxed()
}

async fn admitted_stream(
    id: &str,
    window_seconds: i32,
) -> (
    systemprompt_database::DbPool,
    systemprompt_identifiers::UserId,
    std::sync::Arc<systemprompt_gateway::GatewayAudit>,
    futures::stream::BoxStream<'static, Result<bytes::Bytes, axum::Error>>,
) {
    use futures::StreamExt;
    use systemprompt_gateway::policies::GatewayPolicySpec;
    use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;

    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id.clone()).await;
    let reservation = admitted(
        reserve(
            &repos(&db),
            &user_id,
            &[window("user", window_seconds, None)],
            QuotaFaultMode::Closed,
        )
        .await,
    );
    audit.set_quota_reservation(reservation);
    let body = systemprompt_gateway::stream_tap::tap(
        opened_stream(id, 9),
        crate::stream_tap_pipeline::render(std::sync::Arc::new(AnthropicMessagesInbound)),
        std::sync::Arc::clone(&audit),
        crate::stream_tap_pipeline::tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let mut frames = body.into_data_stream().boxed();
    frames.next().await.expect("first frame").expect("bytes");
    (db, user_id, audit, frames)
}

async fn drained(audit: &systemprompt_gateway::GatewayAudit) {
    assert_eq!(
        audit
            .background()
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );
}

#[tokio::test]
async fn a_client_that_hangs_up_mid_stream_settles_the_usage_it_saw() {
    let (db, user_id, audit, frames) = admitted_stream("resp-exit-drop", 1350).await;
    drop(frames);
    drained(&audit).await;
    assert_eq!(
        bucket(&db, &user_id, 1350).await,
        (1, 9, 0, 0),
        "the reservation is trued down to the partial usage, not kept or freed"
    );
}

#[tokio::test]
async fn a_stream_cut_by_the_shutdown_drain_settles_the_usage_it_saw() {
    use futures::StreamExt;
    let (db, user_id, audit, mut frames) = admitted_stream("resp-exit-drain", 1380).await;
    audit.background().stream_drain().end_streams();
    while let Some(frame) = frames.next().await {
        frame.expect("a drained stream ends cleanly");
    }
    drained(&audit).await;
    assert_eq!(bucket(&db, &user_id, 1380).await, (1, 9, 0, 0));
}
