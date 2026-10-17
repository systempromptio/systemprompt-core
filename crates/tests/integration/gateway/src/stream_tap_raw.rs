//! Pass-through tap framing: an upstream chunk that ends mid-event never
//! leaves the tap's own terminal frame spliced into a partial SSE frame.

use std::sync::Arc;

use bytes::Bytes;
use futures::stream;
use systemprompt_gateway::policies::GatewayPolicySpec;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::stream_tap::{STREAM_ABORT_MESSAGE, tap_raw};
use systemprompt_wire::error::WireStreamError;

use crate::stream_tap_pipeline::{open_audit, settled_status, tap_ctx};
use crate::support::{seed_user, setup_db};

const MESSAGE_START: &str = "event: message_start\ndata: \
     {\"type\":\"message_start\",\"message\":{\"id\":\"msg_raw\",\"type\":\"message\",\
     \"role\":\"assistant\",\"model\":\"claude-test\",\"content\":[],\
     \"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\n";
const PARTIAL_DELTA: &str =
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"ind";

fn assert_well_formed(wire: &str) {
    for frame in wire.split("\n\n").filter(|f| !f.trim().is_empty()) {
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or_else(|| panic!("frame without data: {frame:?}"));
        serde_json::from_str::<serde_json::Value>(data)
            .unwrap_or_else(|e| panic!("frame data is not JSON ({e}): {frame:?}"));
    }
    assert!(wire.ends_with("\n\n"), "{wire:?}");
}

#[tokio::test]
async fn a_chunk_ending_mid_event_then_abort_yields_well_formed_sse() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;

    let chunk = Bytes::from(format!("{MESSAGE_START}{PARTIAL_DELTA}"));
    let upstream = Box::pin(stream::iter(vec![Ok::<Bytes, WireStreamError>(chunk)]));
    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let body = tap_raw(
        upstream,
        inbound,
        Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );

    let bytes = axum::body::to_bytes(body, 1024 * 1024)
        .await
        .expect("collect tapped body");
    let wire = String::from_utf8(bytes.to_vec()).expect("utf8 wire");
    assert!(wire.starts_with(MESSAGE_START), "{wire:?}");
    assert!(!wire.contains("\"ind"), "partial frame leaked: {wire:?}");
    assert!(wire.contains(STREAM_ABORT_MESSAGE), "{wire:?}");
    assert_well_formed(&wire);

    let (status, _) = settled_status(&audit, &db, &ai_request_id).await;
    assert_eq!(status, "failed");
}

#[tokio::test]
async fn a_frame_split_across_chunks_is_released_whole() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;

    let (head, tail) = MESSAGE_START.split_at(40);
    let upstream = Box::pin(stream::iter(vec![
        Ok::<Bytes, WireStreamError>(Bytes::from(head.to_owned())),
        Ok(Bytes::from(tail.to_owned())),
    ]));
    let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let body = tap_raw(
        upstream,
        inbound,
        Arc::clone(&audit),
        tap_ctx(&db, &ai_request_id, GatewayPolicySpec::default()),
    );
    let mut frames = body.into_data_stream();
    let first = futures::StreamExt::next(&mut frames)
        .await
        .expect("first frame")
        .expect("bytes");
    assert_eq!(first, Bytes::from_static(MESSAGE_START.as_bytes()));
}
