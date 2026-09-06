use proptest::prelude::*;
use serde_json::Value;
use systemprompt_models::wire::anthropic::AnthropicStreamState;
use systemprompt_models::wire::sse::frame_end;

// Why: the framing rule the gateway depends on but never stated as a property.
// Upstream bytes arrive in whatever chunks the network produced, and the
// decoder buffers across them. A splitter that mishandles a boundary landing
// inside a frame -- or between the two newlines that end one -- silently drops
// or duplicates an event, and every fixture in the suite feeds whole frames in
// one chunk, so none of them can see it.
fn decode_in_chunks(sse: &str, boundaries: &[usize]) -> Vec<String> {
    let bytes = sse.as_bytes();
    let mut cuts: Vec<usize> = boundaries
        .iter()
        .map(|b| b % bytes.len().max(1))
        .filter(|b| *b > 0)
        .collect();
    cuts.sort_unstable();
    cuts.dedup();

    let mut state = AnthropicStreamState::default();
    let mut buf: Vec<u8> = Vec::new();
    let mut rendered: Vec<String> = Vec::new();
    let mut start = 0;
    for cut in cuts.iter().copied().chain(std::iter::once(bytes.len())) {
        if cut <= start {
            continue;
        }
        buf.extend_from_slice(&bytes[start..cut]);
        start = cut;
        while let Some(end) = frame_end(&buf) {
            let frame: Vec<u8> = buf.drain(..end).collect();
            let text = String::from_utf8_lossy(&frame);
            for line in text.lines() {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    for event in state.events_from_sse(&value) {
                        rendered.push(format!("{event:?}"));
                    }
                }
            }
        }
    }
    rendered
}

fn sample_stream() -> String {
    [
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_p\",\"model\":\"m\",\"usage\":{\"input_tokens\":11}}}\n\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"c1\",\"name\":\"lookup\",\"input\":{}}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\\\"rust\\\"}\"}}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat()
}

proptest! {
    // Chunking is a transport detail. However the bytes are cut up, the decoded
    // event sequence must be the one a single whole-stream chunk produces.
    #[test]
    fn arbitrary_chunk_boundaries_yield_the_same_events(
        boundaries in prop::collection::vec(1usize..4_000, 0..12),
    ) {
        let sse = sample_stream();
        let whole = decode_in_chunks(&sse, &[]);
        let chunked = decode_in_chunks(&sse, &boundaries);
        prop_assert_eq!(whole, chunked);
    }

    // A boundary placed between the two newlines that terminate a frame is the
    // case that breaks a splitter searching for a fixed two-byte separator.
    #[test]
    fn a_boundary_inside_the_frame_terminator_is_still_one_frame(
        offset in 0usize..6,
    ) {
        let sse = sample_stream();
        let whole = decode_in_chunks(&sse, &[]);
        let terminator = sse.find("\n\n").unwrap_or(0) + 1;
        let chunked = decode_in_chunks(&sse, &[terminator.saturating_sub(offset).max(1)]);
        prop_assert_eq!(whole, chunked);
    }
}
