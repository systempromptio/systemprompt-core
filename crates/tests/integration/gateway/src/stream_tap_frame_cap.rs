//! A passthrough stream whose upstream frame grows past the SSE framer's cap
//! ends as a failure with an error frame, and the audit row is failed.

use std::sync::Arc;

use bytes::Bytes;
use futures::stream;
use systemprompt_gateway::policies::GatewayPolicySpec;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::stream_tap::tap_raw;
use systemprompt_wire::error::WireStreamError;
use systemprompt_wire::sse::{MAX_SSE_FRAME_BYTES, SseFrameTooLarge};

use crate::stream_tap_pipeline::{open_audit, settled_status, tap_ctx};
use crate::support::{seed_user, setup_db};

const CHUNK: usize = 1024 * 1024;
const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_big\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"m\",\"content\":[],\"usage\":{\"input_tokens\":4,\"output_tokens\":0}}}\n\n";

fn cap_message() -> String {
    SseFrameTooLarge {
        limit: MAX_SSE_FRAME_BYTES,
    }
    .to_string()
}

async fn run(chunks: Vec<Result<Bytes, WireStreamError>>) -> (Bytes, String, Option<String>) {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;
    let body = tap_raw(
        Box::pin(stream::iter(chunks)),
        Arc::new(AnthropicMessagesInbound),
        Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let wire = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("tapped body");
    let (status, error) = settled_status(&audit, &db, &ai_request_id).await;
    (wire, status, error)
}

#[tokio::test]
async fn a_raw_frame_past_the_framer_cap_fails_with_an_error_frame() {
    let mut chunks = vec![
        Ok::<Bytes, WireStreamError>(Bytes::from_static(START.as_bytes())),
        Ok(Bytes::from_static(b"event: content_block_delta\ndata: ")),
    ];
    for _ in 0..=MAX_SSE_FRAME_BYTES / CHUNK {
        chunks.push(Ok(Bytes::from(vec![b'x'; CHUNK])));
    }
    chunks.push(Ok(Bytes::from_static(
        b"\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    )));

    let (wire, status, error) = run(chunks).await;

    let text = String::from_utf8_lossy(&wire).into_owned();
    assert!(text.starts_with(START), "{text}");
    assert!(text.contains(&cap_message()), "{text}");
    assert!(!text.contains("message_stop"), "{text}");
    assert!(
        wire.len() < CHUNK,
        "oversized frame bytes reached the client"
    );
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some(cap_message().as_str()));
}

#[tokio::test]
async fn an_upstream_frame_cap_error_fails_with_an_error_frame() {
    let chunks = vec![
        Ok::<Bytes, WireStreamError>(Bytes::from_static(START.as_bytes())),
        Err(WireStreamError::from(SseFrameTooLarge {
            limit: MAX_SSE_FRAME_BYTES,
        })),
    ];

    let (wire, status, error) = run(chunks).await;

    let text = String::from_utf8_lossy(&wire).into_owned();
    assert!(text.starts_with(START), "{text}");
    assert!(text.contains(&cap_message()), "{text}");
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some(cap_message().as_str()));
}
