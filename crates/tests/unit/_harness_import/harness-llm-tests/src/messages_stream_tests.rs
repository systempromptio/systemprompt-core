use harness_llm::{Event, FinishReason, LlmError, Protocol, Usage};
use serde_json::json;

use crate::support::decode;

const TOOL_STREAM: &str = concat!(
    "event: message_start\n",
    "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"usage\":{\"input_tokens\":120,\"cache_read_input_tokens\":1000,\"cache_creation_input_tokens\":30,\"output_tokens\":1}}}\n\n",
    "event: ping\n",
    "data: {\"type\":\"ping\"}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Need the file.\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig-abc\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Reading it.\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: content_block_start\n",
    "data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"read\",\"input\":{}}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\": \"}}\n\n",
    "event: content_block_delta\n",
    "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"src/main.rs\\\"}\"}}\n\n",
    "event: content_block_stop\n",
    "data: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
    "event: message_delta\n",
    "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":58}}\n\n",
    "event: message_stop\n",
    "data: {\"type\":\"message_stop\"}\n\n",
);

#[tokio::test]
async fn tool_stream_decodes_to_unified_events() {
    let expected = vec![
        Event::ReasoningDelta {
            text: "Need the file.".into(),
            signature: None,
        },
        Event::ReasoningDelta {
            text: String::new(),
            signature: Some("sig-abc".into()),
        },
        Event::TextDelta("Reading it.".into()),
        Event::ToolCallStart {
            id: "toolu_01".into(),
            name: "read".into(),
        },
        Event::ToolCallDelta {
            id: "toolu_01".into(),
            fragment: "{\"path\": ".into(),
        },
        Event::ToolCallDelta {
            id: "toolu_01".into(),
            fragment: "\"src/main.rs\"}".into(),
        },
        Event::ToolCallEnd {
            id: "toolu_01".into(),
            name: "read".into(),
            input: json!({"path": "src/main.rs"}),
            malformed: None,
        },
        Event::Usage(Usage {
            input: 1_150,
            output: 58,
            reasoning: 0,
            cache_read: 1_000,
            cache_write: 30,
        }),
        Event::Finish {
            reason: FinishReason::ToolCalls,
        },
    ];
    for size in [1, 7, 64, TOOL_STREAM.len()] {
        let events = decode(Protocol::Messages, TOOL_STREAM, size).await.unwrap();
        assert_eq!(events, expected, "chunk size {size}");
    }
}

#[tokio::test]
async fn stream_without_terminal_event_is_truncated() {
    let cut = TOOL_STREAM.split("event: message_delta").next().unwrap();
    assert!(matches!(
        decode(Protocol::Messages, cut, 13).await,
        Err(LlmError::Truncated)
    ));
}

#[tokio::test]
async fn missing_message_stop_still_finishes_after_stop_reason() {
    let cut = TOOL_STREAM.split("event: message_stop").next().unwrap();
    let events = decode(Protocol::Messages, cut, 9).await.unwrap();
    assert_eq!(
        events.last(),
        Some(&Event::Finish {
            reason: FinishReason::ToolCalls
        })
    );
}

#[tokio::test]
async fn in_stream_errors_surface() {
    let stream = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
    assert!(matches!(
        decode(Protocol::Messages, stream, 5).await,
        Err(LlmError::Provider { kind: Some(ref kind), ref message })
            if kind == "overloaded_error" && message == "Overloaded"
    ));
}

#[tokio::test]
async fn redacted_reasoning_and_unknown_stop_reason_pass_through() {
    let stream = concat!(
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"opaque\"}}\n\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"something_new\"}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let events = decode(Protocol::Messages, stream, 11).await.unwrap();
    assert_eq!(
        events,
        vec![
            Event::RedactedReasoning {
                data: "opaque".into()
            },
            Event::Finish {
                reason: FinishReason::Other("something_new".into())
            },
        ]
    );
}
