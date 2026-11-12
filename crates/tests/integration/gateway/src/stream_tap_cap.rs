//! A tapped stream whose content passes the accumulation cap ends as a failure
//! with an error frame, so no consumer reads a truncated snapshot as whole.

use std::sync::Arc;

use bytes::Bytes;
use futures::stream;
use systemprompt_gateway::policies::GatewayPolicySpec;
use systemprompt_gateway::protocol::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::stream_tap::budget::{
    ACCUMULATION_CAP_MESSAGE, MAX_ACCUMULATED_RESPONSE_BYTES,
};
use systemprompt_gateway::stream_tap::{tap, tap_raw};
use systemprompt_wire::error::WireStreamError;

use crate::stream_tap_pipeline::{events_stream, open_audit, render, settled_status, tap_ctx};
use crate::support::{seed_user, setup_db};

const CHUNK: usize = 1024 * 1024;

fn chunks_past_the_cap() -> usize {
    MAX_ACCUMULATED_RESPONSE_BYTES / CHUNK + 2
}

#[tokio::test]
async fn a_canonical_stream_past_the_content_cap_fails_with_an_error_frame() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;
    let mut events = vec![
        Ok(CanonicalEvent::MessageStart {
            id: "resp-huge".to_owned(),
            model: "claude-served".to_owned(),
            usage: CanonicalUsage {
                input_tokens: 4,
                ..CanonicalUsage::default()
            },
        }),
        Ok(CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        }),
    ];
    for _ in 0..chunks_past_the_cap() {
        events.push(Ok(CanonicalEvent::TextDelta {
            index: 0,
            text: "h".repeat(CHUNK),
        }));
    }
    events.push(Ok(CanonicalEvent::ContentBlockStop { index: 0 }));
    events.push(Ok(CanonicalEvent::MessageStop {
        id: "resp-huge".to_owned(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
        raw_finish_reason: None,
    }));
    let body = tap(
        events_stream(events),
        render(Arc::new(AnthropicMessagesInbound)),
        Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let wire = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("tapped body");
    let tail = String::from_utf8_lossy(&wire[wire.len().saturating_sub(512)..]).into_owned();
    assert!(tail.contains(ACCUMULATION_CAP_MESSAGE), "{tail}");
    assert!(!tail.contains("message_stop"), "{tail}");
    assert!(wire.len() < MAX_ACCUMULATED_RESPONSE_BYTES + 2 * CHUNK);

    let (status, error) = settled_status(&audit, &db, &ai_request_id).await;
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some(ACCUMULATION_CAP_MESSAGE));
}

#[tokio::test]
async fn a_raw_stream_past_the_content_cap_fails_with_an_error_frame() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;
    let start = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_raw\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"m\",\"content\":[],\"usage\":{\"input_tokens\":4,\"output_tokens\":0}}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
    let delta = Bytes::from(format!(
        "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{}\"}}}}\n\n",
        "r".repeat(CHUNK)
    ));
    let mut chunks = vec![Ok::<Bytes, WireStreamError>(Bytes::from_static(
        start.as_bytes(),
    ))];
    for _ in 0..chunks_past_the_cap() {
        chunks.push(Ok(delta.clone()));
    }
    chunks.push(Ok(Bytes::from_static(
        b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    )));
    let body = tap_raw(
        Box::pin(stream::iter(chunks)),
        Arc::new(AnthropicMessagesInbound),
        Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let wire = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("tapped body");
    let tail = String::from_utf8_lossy(&wire[wire.len().saturating_sub(512)..]).into_owned();
    assert!(tail.contains(ACCUMULATION_CAP_MESSAGE), "{tail}");
    assert!(!tail.contains("message_stop"), "{tail}");

    let (status, error) = settled_status(&audit, &db, &ai_request_id).await;
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some(ACCUMULATION_CAP_MESSAGE));
}
