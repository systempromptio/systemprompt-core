use serde_json::json;
use systemprompt_engine::llm::{Event, FinishReason, LlmError, Protocol, Usage};

use crate::llm::support::decode;

const TOOL_STREAM: &str = concat!(
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\r\n\r\n",
    ": processing\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"Two files.\"},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Checking both.\"},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\":\"}}]},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_b\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"b.rs\\\"}\"}}]},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"a.rs\\\"}\"}}]},\"finish_reason\":null}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\r\n\r\n",
    "data: {\"id\":\"c1\",\"choices\":[],\"usage\":{\"prompt_tokens\":900,\"completion_tokens\":40,\"total_tokens\":940,\"prompt_tokens_details\":{\"cached_tokens\":512},\"completion_tokens_details\":{\"reasoning_tokens\":12}}}\r\n\r\n",
    "data: [DONE]\r\n\r\n",
);

#[tokio::test]
async fn tool_stream_decodes_to_unified_events() {
    let expected = vec![
        Event::ReasoningDelta {
            text: "Two files.".into(),
            signature: None,
        },
        Event::TextDelta("Checking both.".into()),
        Event::ToolCallStart {
            id: "call_a".into(),
            name: "read".into(),
        },
        Event::ToolCallDelta {
            id: "call_a".into(),
            fragment: "{\"path\":".into(),
        },
        Event::ToolCallStart {
            id: "call_b".into(),
            name: "read".into(),
        },
        Event::ToolCallDelta {
            id: "call_b".into(),
            fragment: "{\"path\":\"b.rs\"}".into(),
        },
        Event::ToolCallDelta {
            id: "call_a".into(),
            fragment: "\"a.rs\"}".into(),
        },
        Event::ToolCallEnd {
            id: "call_a".into(),
            name: "read".into(),
            input: json!({"path": "a.rs"}),
            malformed: None,
        },
        Event::ToolCallEnd {
            id: "call_b".into(),
            name: "read".into(),
            input: json!({"path": "b.rs"}),
            malformed: None,
        },
        Event::Usage(Usage {
            input: 900,
            output: 40,
            reasoning: 12,
            cache_read: 512,
            cache_write: 0,
        }),
        Event::Finish {
            reason: FinishReason::ToolCalls,
        },
    ];
    for size in [1, 5, 33, TOOL_STREAM.len()] {
        let events = decode(Protocol::ChatCompletions, TOOL_STREAM, size)
            .await
            .unwrap();
        assert_eq!(events, expected, "chunk size {size}");
    }
}

#[tokio::test]
async fn stream_without_finish_or_done_is_truncated() {
    let marker = "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},";
    let cut = TOOL_STREAM.split(marker).next().unwrap();
    assert!(matches!(
        decode(Protocol::ChatCompletions, cut, 11).await,
        Err(LlmError::Truncated)
    ));
}

#[tokio::test]
async fn missing_done_still_finishes_after_finish_reason() {
    let stream = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":\"stop\"}]}\n\n";
    let events = decode(Protocol::ChatCompletions, stream, 3).await.unwrap();
    assert_eq!(
        events,
        vec![
            Event::TextDelta("hi".into()),
            Event::Finish {
                reason: FinishReason::Stop
            }
        ]
    );
}

#[tokio::test]
async fn malformed_arguments_keep_raw_text() {
    let stream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"x\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"cmd\\\": \"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let events = decode(Protocol::ChatCompletions, stream, 4).await.unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolCallEnd { malformed: Some(raw), .. } if raw == "{\"cmd\": "
    )));
}

#[tokio::test]
async fn in_stream_error_surfaces() {
    let stream = "data: {\"error\":{\"message\":\"maximum context length is 8192 tokens\",\"code\":\"context_length_exceeded\"}}\n\n";
    let result = decode(Protocol::ChatCompletions, stream, 6).await;
    assert!(matches!(
        result,
        Err(LlmError::ContextOverflow { status: None, .. })
    ));
}

#[tokio::test]
async fn unindexed_tool_calls_get_their_own_slots() {
    let stream = concat!(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"a\",\"function\":{\"name\":\"ls\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"b\",\"function\":{\"name\":\"pwd\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let events = decode(Protocol::ChatCompletions, stream, 8).await.unwrap();
    let ends: Vec<(&str, &str)> = events
        .iter()
        .filter_map(|e| match e {
            Event::ToolCallEnd { id, name, .. } => Some((id.as_str(), name.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(ends, vec![("a", "ls"), ("b", "pwd")]);
}
