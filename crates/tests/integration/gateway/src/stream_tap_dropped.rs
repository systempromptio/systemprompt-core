//! A passthrough stream releases only frames the tap decoded: an undecodable
//! data frame is withheld and counted, a well-formed frame of an unknown type
//! is released, and a stream with no decodable frame ends as a failure.

use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::stream;
use systemprompt_gateway::policies::GatewayPolicySpec;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::stream_tap::tap_raw;
use systemprompt_wire::error::WireStreamError;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

use crate::stream_tap_pipeline::{open_audit, settled_status, tap_ctx};
use crate::support::{seed_user, setup_db};

const START: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_d\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"m\",\"content\":[],\"usage\":{\"input_tokens\":4,\"output_tokens\":0}}}\n\n";
const BLOCK: &str = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"visible\"}}\n\n";
const STOP: &str = "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
const GARBAGE: &str = "event: content_block_delta\ndata: {not json \"hidden\"\n\n";
const SHAPELESS: &str = "data: [\"hidden\"]\n\n";
const UNKNOWN: &str =
    "event: future_event\ndata: {\"type\":\"future_event\",\"note\":\"accounted\"}\n\n";

#[derive(Clone, Default)]
struct DroppedFrames(Arc<Mutex<Vec<u64>>>);

struct DroppedVisitor(Option<u64>);

impl Visit for DroppedVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "dropped_frames" {
            self.0 = Some(value);
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {}
}

impl<S: Subscriber> Layer<S> for DroppedFrames {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = DroppedVisitor(None);
        event.record(&mut visitor);
        if let Some(count) = visitor.0 {
            self.0.lock().expect("capture").push(count);
        }
    }
}

struct Outcome {
    wire: String,
    status: String,
    error: Option<String>,
    dropped: Vec<u64>,
}

async fn run(frames: &[&str]) -> Outcome {
    let capture = DroppedFrames::default();
    let _guard =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(capture.clone()));
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let (audit, ai_request_id) = open_audit(&db, user_id).await;
    let chunks: Vec<Result<Bytes, WireStreamError>> = frames
        .iter()
        .map(|f| Ok(Bytes::copy_from_slice(f.as_bytes())))
        .collect();
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
    let dropped = capture.0.lock().expect("capture").clone();
    Outcome {
        wire: String::from_utf8_lossy(&wire).into_owned(),
        status,
        error,
        dropped,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn an_undecodable_frame_between_good_ones_is_withheld_and_counted() {
    let out = run(&[START, GARBAGE, BLOCK, SHAPELESS, STOP]).await;

    assert_eq!(out.wire, [START, BLOCK, STOP].concat());
    assert!(!out.wire.contains("hidden"));
    assert_eq!(out.status, "completed");
    assert_eq!(out.dropped, vec![2]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_well_formed_frame_of_an_unknown_type_is_released() {
    let out = run(&[START, UNKNOWN, BLOCK, STOP]).await;

    assert_eq!(out.wire, [START, UNKNOWN, BLOCK, STOP].concat());
    assert_eq!(out.status, "completed");
    assert!(out.dropped.is_empty(), "{:?}", out.dropped);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stream_with_no_decodable_frame_fails_with_an_error_frame() {
    let out = run(&[GARBAGE, SHAPELESS]).await;

    assert!(!out.wire.contains("hidden"), "{}", out.wire);
    assert!(out.wire.contains("no decodable frame"), "{}", out.wire);
    assert_eq!(out.status, "failed");
    assert!(
        out.error
            .as_deref()
            .is_some_and(|e| e.contains("no decodable frame")),
        "{:?}",
        out.error
    );
    assert_eq!(out.dropped, vec![2]);
}
