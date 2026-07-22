//! End-to-end taps through `stream_tap::tap`: canonical upstream events are
//! re-rendered to the client wire while the audit sink lands the terminal
//! `ai_requests` state for completion, upstream error, and abandoned streams.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::stream;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::stream_tap::tap;
use systemprompt_api::services::gateway::{GatewayAudit, GatewayRequestContext};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};

use crate::support::{minimal_request, seed_user, setup_db};

fn usage(input: u32, output: u32) -> CanonicalUsage {
    CanonicalUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        total_tokens: input + output,
    }
}

async fn open_audit(db: &DbPool, user_id: UserId) -> (Arc<GatewayAudit>, AiRequestId) {
    let request = minimal_request(Some("tap-system"), "tap first turn");
    let gw_conv = request
        .derived_gateway_conversation_id()
        .expect("gateway conversation id");
    let context_id = ContextId::derived_from_gateway_conversation(&gw_conv);
    let ai_request_id = AiRequestId::generate();
    let ctx = GatewayRequestContext {
        ai_request_id: ai_request_id.clone(),
        user_id,
        session_id: None,
        context_id,
        gateway_conversation_id: Some(gw_conv),
        trace_id: None,
        provider: "anthropic".to_string(),
        requested_model: Some("claude-requested".to_string()),
        model: "claude-test".to_string(),
        max_tokens: Some(16),
        is_streaming: true,
        wire_protocol: "anthropic-messages".to_string(),
    };
    let audit = GatewayAudit::new(db, ctx).expect("audit ctor");
    audit
        .open(&request, &Bytes::from_static(b"{\"stream\":true}"))
        .await
        .expect("audit open");
    (Arc::new(audit), ai_request_id)
}

async fn wait_for_terminal_status(db: &DbPool, id: &AiRequestId) -> (String, Option<String>) {
    let pool = db.pool_arc().expect("read pool");
    for _ in 0..200 {
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT status, error_message FROM ai_requests WHERE id = $1")
                .bind(id.as_str())
                .fetch_optional(pool.as_ref())
                .await
                .expect("query ai_requests");
        if let Some((status, error)) = row {
            if status != "pending" && status != "processing" {
                return (status, error);
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("ai_requests row never reached a terminal status");
}

fn events_stream(
    events: Vec<Result<CanonicalEvent, String>>,
) -> futures::stream::BoxStream<'static, Result<CanonicalEvent, String>> {
    Box::pin(stream::iter(events))
}

#[tokio::test]
async fn tap_renders_client_bytes_and_completes_audit_on_eof() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;

    let upstream = events_stream(vec![
        Ok(CanonicalEvent::MessageStart {
            id: "resp-tap-1".to_owned(),
            model: "claude-served".to_owned(),
            usage: usage(10, 0),
        }),
        Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }),
        Ok(CanonicalEvent::TextDelta {
            index: 0,
            text: "Hello from tap".to_owned(),
        }),
        Ok(CanonicalEvent::ContentBlockStop { index: 0 }),
        Ok(CanonicalEvent::MessageStop {
            id: "resp-tap-1".to_owned(),
            stop_reason: Some(CanonicalStopReason::EndTurn),
        }),
        Ok(CanonicalEvent::UsageDelta(usage(0, 7))),
    ]);
    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let body = tap(
        upstream,
        inbound,
        "claude-test".to_owned(),
        Arc::clone(&audit),
    );

    let bytes = axum::body::to_bytes(body, 4 * 1024 * 1024)
        .await
        .expect("collect tapped body");
    let wire = String::from_utf8(bytes.to_vec()).expect("utf8 wire");
    assert!(wire.contains("message_start"), "{wire}");
    assert!(wire.contains("Hello from tap"), "{wire}");
    assert!(wire.contains("message_stop"), "{wire}");

    let (status, error) = wait_for_terminal_status(&db, &ai_request_id).await;
    assert_eq!(status, "completed", "error: {error:?}");
    assert!(error.is_none(), "{error:?}");

    let pool = db.pool_arc().expect("read pool");
    let (input_tokens, output_tokens, model): (Option<i32>, Option<i32>, Option<String>) =
        sqlx::query_as("SELECT input_tokens, output_tokens, model FROM ai_requests WHERE id = $1")
            .bind(ai_request_id.as_str())
            .fetch_one(pool.as_ref())
            .await
            .expect("fetch usage");
    assert_eq!(input_tokens, Some(10));
    assert_eq!(output_tokens, Some(7));
    assert_eq!(
        model.as_deref(),
        Some("claude-served"),
        "served model must be recorded on the audit row"
    );
}

#[tokio::test]
async fn tap_surfaces_upstream_error_to_client_and_fails_audit() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;

    let upstream = events_stream(vec![
        Ok(CanonicalEvent::MessageStart {
            id: "resp-tap-2".to_owned(),
            model: "claude-served".to_owned(),
            usage: usage(3, 0),
        }),
        Err("upstream exploded".to_owned()),
    ]);
    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let body = tap(
        upstream,
        inbound,
        "claude-test".to_owned(),
        Arc::clone(&audit),
    );

    let collected = axum::body::to_bytes(body, 4 * 1024 * 1024).await;
    assert!(
        collected.is_err(),
        "upstream stream error must break the client body"
    );

    let (status, error) = wait_for_terminal_status(&db, &ai_request_id).await;
    assert_eq!(status, "failed");
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("upstream exploded")),
        "{error:?}"
    );
}

#[tokio::test]
async fn tap_dropped_before_polling_fails_audit_as_empty_stream() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;

    let upstream = events_stream(vec![Ok(CanonicalEvent::MessageStart {
        id: "resp-tap-3".to_owned(),
        model: "claude-served".to_owned(),
        usage: usage(2, 0),
    })]);
    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let body = tap(
        upstream,
        inbound,
        "claude-test".to_owned(),
        Arc::clone(&audit),
    );
    drop(body);

    let (status, error) = wait_for_terminal_status(&db, &ai_request_id).await;
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some("empty upstream stream"));
}
