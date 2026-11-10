use bytes::Bytes;
use futures::StreamExt;
use futures::executor::block_on;
use systemprompt_wire::canonical::{CanonicalEvent, CanonicalStopReason};
use systemprompt_wire::error::WireStreamError;
use systemprompt_wire::{anthropic, gemini, openai_chat, openai_responses};

#[derive(Clone, Copy, Debug)]
enum Codec {
    Anthropic,
    OpenAiChat,
    OpenAiResponses,
    Gemini,
}

type Rendered = Vec<Result<String, String>>;

fn render(event: Result<CanonicalEvent, WireStreamError>) -> Result<String, String> {
    match event {
        Ok(CanonicalEvent::MessageStart { model, .. }) => Ok(format!("start {model}")),
        Ok(CanonicalEvent::MessageStop {
            stop_reason,
            raw_finish_reason,
            ..
        }) => Ok(format!("stop {stop_reason:?} {raw_finish_reason:?}")),
        Ok(other) => Ok(format!("{other:?}")),
        Err(e) => Err(e.to_string()),
    }
}

fn run(codec: Codec, chunks: Vec<Vec<u8>>) -> Rendered {
    let stream = futures::stream::iter(
        chunks
            .into_iter()
            .map(|c| Ok::<_, std::io::Error>(Bytes::from(c))),
    );
    let events = match codec {
        Codec::Anthropic => anthropic::sse_to_canonical_events(stream),
        Codec::OpenAiChat => openai_chat::sse_to_canonical_events(stream, "m".to_owned()),
        Codec::OpenAiResponses => openai_responses::sse_to_canonical_events(stream, "m".to_owned()),
        Codec::Gemini => gemini::sse_to_canonical_events(stream, "m".to_owned()),
    };
    block_on(events.map(render).collect::<Vec<_>>())
}

fn whole(codec: Codec, sse: &str) -> Rendered {
    run(codec, vec![sse.as_bytes().to_vec()])
}

fn split(codec: Codec, sse: &str, cut: usize) -> Rendered {
    let bytes = sse.as_bytes();
    run(codec, vec![bytes[..cut].to_vec(), bytes[cut..].to_vec()])
}

const ANTHROPIC: &str = concat!(
    "event: message_start\r\ndata:{\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude\",\"usage\":{\"input_tokens\":3}}}\r\n\r\n",
    ": ping\r\n\r\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "data:{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"héllo 世界\"}}\n\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "data:{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\n",
    "data:{\"type\":\"message_stop\"}\n\n",
);

const OPENAI_CHAT: &str = concat!(
    "data:{\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"content\":\"héllo \"}}]}\r\n\r\n",
    ": keepalive\n\n",
    "data:{\"id\":\"c1\",\"choices\":[{\"delta\":{\"content\":\"世界\"},\"finish_reason\":\"stop\"}]}\n\n",
    "data:{\"id\":\"c1\",\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\n",
    "data:[DONE]\n\n",
);

const OPENAI_RESPONSES: &str = concat!(
    "event: response.created\ndata:{\"type\":\"response.created\",\"response\":{\"id\":\"r1\",\"model\":\"gpt\"}}\n\n",
    "data:{\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\"}}\r\n\r\n",
    "data:{\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"héllo 世界\"}\n\n",
    "data:{\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\"}}\n\n",
    "data:{\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"usage\":{\"input_tokens\":4,\"output_tokens\":2,\"total_tokens\":6}}}\n\n",
);

const GEMINI: &str = concat!(
    "data:{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"héllo \"}]}}]}\r\n\r\n",
    "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"世界\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":4,\"candidatesTokenCount\":2,\"totalTokenCount\":6}}\r\n\r\n",
);

fn fixtures() -> [(Codec, &'static str); 4] {
    [
        (Codec::Anthropic, ANTHROPIC),
        (Codec::OpenAiChat, OPENAI_CHAT),
        (Codec::OpenAiResponses, OPENAI_RESPONSES),
        (Codec::Gemini, GEMINI),
    ]
}

#[test]
fn every_codec_decodes_unspaced_data_crlf_and_comments_to_a_full_turn() {
    for (codec, sse) in fixtures() {
        let events = whole(codec, sse);
        assert!(events.iter().all(Result::is_ok), "{codec:?}: {events:?}");
        let text: String = events
            .iter()
            .flatten()
            .filter(|e| e.starts_with("TextDelta"))
            .cloned()
            .collect();
        assert!(text.contains("héllo"), "{codec:?}: {events:?}");
        assert!(text.contains("世界"), "{codec:?}: {events:?}");
        assert!(
            events.iter().flatten().any(|e| e.starts_with("stop ")),
            "{codec:?}: no stop in {events:?}"
        );
    }
}

#[test]
fn every_codec_yields_the_same_events_at_every_chunk_boundary() {
    for (codec, sse) in fixtures() {
        let expected = whole(codec, sse);
        for cut in 0..=sse.len() {
            assert_eq!(split(codec, sse, cut), expected, "{codec:?} cut at {cut}");
        }
    }
}

#[test]
fn every_codec_ends_an_oversized_frame_with_a_typed_error() {
    let long = format!("data: {{\"x\":\"{}\"", "a".repeat(17 * 1024 * 1024));
    for (codec, _) in fixtures() {
        let events = whole(codec, &long);
        assert_eq!(events.len(), 1, "{codec:?}: {events:?}");
        let message = events[0].clone().expect_err("frame error");
        assert!(message.contains("exceeds"), "{codec:?}: {message}");
    }
}

#[test]
fn undecodable_and_mis_shaped_frames_are_skipped_by_every_codec() {
    let cases = [
        (Codec::Anthropic, ANTHROPIC, "[1,2]"),
        (Codec::OpenAiChat, OPENAI_CHAT, "[1]"),
        (Codec::OpenAiResponses, OPENAI_RESPONSES, "{\"x\":1}"),
        (Codec::Gemini, GEMINI, "{\"candidates\":\"nope\"}"),
    ];
    for (codec, sse, wrong_shape) in cases {
        let clean = whole(codec, sse);
        for bad in ["{not json", wrong_shape] {
            let (head, tail) = sse.split_at(sse.find("\n\n").map_or(0, |i| i + 2));
            let noisy = format!("data: {bad}\n\n{head}data: {bad}\n\n{tail}");
            assert_eq!(whole(codec, &noisy), clean, "{codec:?} with {bad}");
        }
    }
}

fn chat_tool_turn(chunks: &[&str]) -> Vec<CanonicalEvent> {
    let sse: String = chunks.iter().map(|c| format!("data: {c}\n\n")).collect();
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(sse)) });
    block_on(
        openai_chat::sse_to_canonical_events(stream, "m".to_owned())
            .map(|e| e.expect("event"))
            .collect::<Vec<_>>(),
    )
}

fn stop_reason(events: &[CanonicalEvent]) -> Option<CanonicalStopReason> {
    events.iter().find_map(|e| match e {
        CanonicalEvent::MessageStop { stop_reason, .. } => *stop_reason,
        _ => None,
    })
}

fn tool_blocks(events: &[CanonicalEvent]) -> Vec<(u32, String, String)> {
    let mut blocks: Vec<(u32, String, String)> = Vec::new();
    for event in events {
        match event {
            CanonicalEvent::ContentBlockStart {
                index,
                block: systemprompt_wire::canonical::ContentBlockKind::ToolUse { id, .. },
            } => blocks.push((*index, id.clone(), String::new())),
            CanonicalEvent::ToolUseDelta {
                index,
                partial_json,
            } => {
                if let Some(block) = blocks.iter_mut().find(|b| b.0 == *index) {
                    block.2.push_str(partial_json);
                }
            },
            _ => {},
        }
    }
    blocks
}

#[test]
fn an_empty_tool_calls_array_does_not_force_a_tool_use_stop() {
    let events = chat_tool_turn(&[
        r#"{"id":"c","choices":[{"delta":{"content":"hi","tool_calls":[]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[]},"finish_reason":"stop"}]}"#,
    ]);
    assert!(tool_blocks(&events).is_empty());
    assert_eq!(stop_reason(&events), Some(CanonicalStopReason::EndTurn));
}

#[test]
fn parallel_calls_without_an_index_are_keyed_by_id() {
    let events = chat_tool_turn(&[
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"id":"a","function":{"name":"f","arguments":"{\"x\":"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"function":{"arguments":"1}"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"id":"b","function":{"name":"g","arguments":"{\"y\":2}"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
    ]);
    let blocks = tool_blocks(&events);
    assert_eq!(blocks.len(), 2, "{events:?}");
    assert_eq!(blocks[0].1, "a");
    assert_eq!(blocks[0].2, r#"{"x":1}"#);
    assert_eq!(blocks[1].1, "b");
    assert_eq!(blocks[1].2, r#"{"y":2}"#);
    assert_eq!(stop_reason(&events), Some(CanonicalStopReason::ToolUse));
}

#[test]
fn interleaved_indexed_deltas_assemble_each_call() {
    let events = chat_tool_turn(&[
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"f","arguments":""}},{"index":1,"id":"b","function":{"name":"g","arguments":"{\"q\""}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"p\":1}"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":":2}"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
    ]);
    let blocks = tool_blocks(&events);
    assert_eq!(blocks.len(), 2, "{events:?}");
    assert_eq!(blocks[0].2, r#"{"p":1}"#);
    assert_eq!(blocks[1].2, r#"{"q":2}"#);
}

#[test]
fn a_call_finished_without_arguments_carries_an_empty_object() {
    let events = chat_tool_turn(&[
        r#"{"id":"c","choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"now"}}]}}]}"#,
        r#"{"id":"c","choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
    ]);
    let blocks = tool_blocks(&events);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].2, "{}");
    let parsed: serde_json::Value = serde_json::from_str(&blocks[0].2).expect("json");
    assert!(parsed.is_object());
}

#[test]
fn usage_counts_beyond_u32_saturate() {
    let events = chat_tool_turn(&[
        r#"{"id":"c","choices":[{"delta":{"content":"x"},"finish_reason":"stop"}]}"#,
        r#"{"id":"c","choices":[],"usage":{"prompt_tokens":4294967301,"completion_tokens":4294967296}}"#,
    ]);
    let usage = events
        .iter()
        .find_map(|e| match e {
            CanonicalEvent::UsageDelta(u) => Some(u.clone()),
            _ => None,
        })
        .expect("usage");
    assert_eq!(usage.input_tokens, Some(u32::MAX));
    assert_eq!(usage.output_tokens, Some(u32::MAX));
}
