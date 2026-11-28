//! The chat-completions request body: assistant turns, tool calls and results,
//! media, tool choice and output limits.

use serde_json::json;
use systemprompt_engine::llm::{
    Attachment, Content, Message, Protocol, Reasoning, ReasoningEffort, Supports, ToolChoice,
    ToolDef,
};

use url::Url;

use crate::support::{all_supports, body, request};

fn tool(name: &str) -> ToolDef {
    ToolDef {
        name: name.into(),
        description: "d".into(),
        input_schema: json!({"type": "object"}),
    }
}

#[test]
fn empty_and_whitespace_only_assistant_messages_are_omitted() {
    let mut req = request(Protocol::ChatCompletions, Supports::default());
    req.messages = vec![
        Message::user_text("q"),
        Message::assistant(vec![]),
        Message::assistant(vec![Content::text("  ")]),
        Message::user_text("again"),
    ];
    let messages = body(&req)["messages"].as_array().unwrap().clone();
    assert!(messages.iter().all(|m| m["role"] != "assistant"));
}

#[test]
fn assistant_text_parts_are_joined_without_separators() {
    let mut req = request(Protocol::ChatCompletions, Supports::default());
    req.messages = vec![Message::assistant(vec![
        Content::text("Hel"),
        Content::text("lo"),
    ])];
    assert_eq!(body(&req)["messages"][0]["content"], "Hello");
}

#[test]
fn tool_calls_and_results_are_lowered_to_assistant_tool_calls_and_tool_messages() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![
        Message::user_text("go"),
        Message::assistant(vec![Content::ToolCall {
            id: "c1".into(),
            name: "read".into(),
            input: json!({"path": "a"}),
        }]),
        Message::user(vec![Content::ToolResult {
            id: "c1".into(),
            output: "error: no such file".into(),
            is_error: true,
            attachments: vec![],
        }]),
    ];
    let messages = body(&req)["messages"].clone();
    assert_eq!(
        messages[1]["tool_calls"][0]["function"],
        json!({"name": "read", "arguments": "{\"path\":\"a\"}"})
    );
    assert_eq!(
        messages[2],
        json!({"role": "tool", "tool_call_id": "c1", "content": "error: no such file"})
    );
}

#[test]
fn a_historical_tool_call_without_input_is_sent_with_empty_object_arguments() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::assistant(vec![
        Content::ToolCall {
            id: "c1".into(),
            name: "list".into(),
            input: serde_json::Value::Null,
        },
        Content::ToolCall {
            id: "c2".into(),
            name: "read".into(),
            input: json!({"path": "a"}),
        },
    ])];
    let calls = body(&req)["messages"][0]["tool_calls"].clone();
    assert_eq!(calls[0]["function"]["arguments"], "{}");
    assert_eq!(calls[1]["function"]["arguments"], "{\"path\":\"a\"}");
}

#[test]
fn image_tool_results_follow_all_tool_messages_as_one_vision_message() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    let image = |data: &str| Attachment {
        mime: "image/png".into(),
        data: data.into(),
        name: None,
    };
    req.messages = vec![Message::user(vec![
        Content::ToolResult {
            id: "a".into(),
            output: "first".into(),
            is_error: false,
            attachments: vec![image("AAA")],
        },
        Content::ToolResult {
            id: "b".into(),
            output: "second".into(),
            is_error: false,
            attachments: vec![image("BBB")],
        },
    ])];
    let messages = body(&req)["messages"].as_array().unwrap().clone();
    assert_eq!(messages.len(), 3);
    assert_eq!(
        (messages[0]["role"].as_str(), messages[1]["role"].as_str()),
        (Some("tool"), Some("tool"))
    );
    assert!(!messages[0]["content"].as_str().unwrap().contains("AAA"));
    let parts = messages[2]["content"].as_array().unwrap();
    assert_eq!(parts[0]["image_url"]["url"], "data:image/png;base64,AAA");
    assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,BBB");
}

#[test]
fn encoded_user_images_are_passed_through_as_data_urls() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::user(vec![
        Content::text("see"),
        Content::Image {
            mime: "image/gif".into(),
            data: "R0lG".into(),
        },
    ])];
    assert_eq!(
        body(&req)["messages"][0]["content"][1],
        json!({"type": "image_url", "image_url": {"url": "data:image/gif;base64,R0lG"}})
    );
}

#[test]
fn an_inline_pdf_is_sent_as_a_file_part() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::user(vec![Content::Document {
        mime: "application/pdf".into(),
        data: "JVBE".into(),
        name: Some("r.pdf".into()),
    }])];
    assert_eq!(
        body(&req)["messages"][0]["content"][0],
        json!({"type": "file", "file": {"filename": "r.pdf", "file_data": "data:application/pdf;base64,JVBE"}})
    );
}

#[test]
fn a_pdf_returned_by_a_tool_moves_into_a_following_user_message() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::user(vec![Content::ToolResult {
        id: "t".into(),
        output: "report".into(),
        is_error: false,
        attachments: vec![Attachment {
            mime: "application/pdf".into(),
            data: "JVBE".into(),
            name: None,
        }],
    }])];
    let messages = body(&req)["messages"].clone();
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"][0]["type"], "file");
}

#[test]
#[ignore = "parity ledger: a non-image attachment the protocol cannot carry becomes a notice instead of refusing the request"]
fn media_that_cannot_be_lowered_is_refused() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::user(vec![Content::Document {
        mime: "audio/wav".into(),
        data: "UklG".into(),
        name: None,
    }])];
    let text = body(&req)["messages"][0]["content"].to_string();
    assert!(!text.contains("omitted"));
}

#[test]
#[ignore = "parity ledger: reasoning in history is not replayed on the chat-completions protocol"]
fn reasoning_only_assistant_history_is_replayed() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![
        Message::user_text("q"),
        Message::assistant(vec![Content::Reasoning {
            text: "thinking".into(),
            signature: None,
        }]),
    ];
    assert!(body(&req)["messages"].to_string().contains("thinking"));
}

#[test]
#[ignore = "parity ledger: replayed tool-call ids and names are sent as stored, characters the endpoint rejects included"]
fn replayed_tool_call_ids_are_cleaned_of_rejected_characters() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.messages = vec![Message::assistant(vec![Content::ToolCall {
        id: "call:1/x".into(),
        name: "my tool!".into(),
        input: json!({}),
    }])];
    let call = body(&req)["messages"][0]["tool_calls"][0].clone();
    assert_eq!(call["function"]["name"], "my_tool_");
}

#[test]
fn the_tool_choice_is_sent_only_when_tools_are_offered() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.params.tool_choice = ToolChoice::Required;
    req.messages = vec![Message::assistant(vec![Content::ToolCall {
        id: "c".into(),
        name: "read".into(),
        input: json!({}),
    }])];
    assert!(body(&req).get("tool_choice").is_none());
    req.tools = vec![tool("read")];
    assert_eq!(body(&req)["tool_choice"], "required");
}

#[test]
fn a_basic_body_streams_with_usage_and_a_tool_body_lists_functions() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.system = vec!["sys".into()];
    req.messages = vec![Message::user_text("hi")];
    let basic = body(&req);
    assert_eq!(basic["stream"], true);
    assert_eq!(basic["stream_options"], json!({"include_usage": true}));
    assert_eq!(
        basic["messages"][0],
        json!({"role": "system", "content": "sys"})
    );
    assert_eq!(basic["max_tokens"], 8_000);
    req.tools = vec![tool("read")];
    assert_eq!(
        body(&req)["tools"][0],
        json!({"type": "function", "function": {"name": "read", "description": "d", "parameters": {"type": "object"}}})
    );
}

#[test]
fn reasoning_efforts_are_spelled_as_reasoning_effort() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    for (reasoning, spelled) in [
        (Reasoning::Effort(ReasoningEffort::Low), "low"),
        (Reasoning::Effort(ReasoningEffort::Medium), "medium"),
        (Reasoning::Effort(ReasoningEffort::Max), "high"),
        (Reasoning::Budget(1_000), "low"),
        (Reasoning::Budget(10_000), "medium"),
        (Reasoning::Budget(50_000), "high"),
    ] {
        req.params.reasoning = Some(reasoning);
        assert_eq!(body(&req)["reasoning_effort"], spelled, "{reasoning:?}");
    }
}

#[test]
fn no_cache_markers_are_sent_on_the_chat_completions_protocol() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    req.system = vec!["s".into()];
    req.tools = vec![tool("read")];
    req.messages = vec![
        Message::user_text("a"),
        Message::assistant(vec![Content::text("b")]),
    ];
    assert!(!body(&req).to_string().contains("cache_control"));
}

#[test]
#[ignore = "parity ledger: two tools with the same name are both declared; the last definition does not replace the first"]
fn the_last_definition_of_a_duplicated_tool_name_is_kept() {
    let mut req = request(Protocol::ChatCompletions, all_supports());
    let mut second = tool("read");
    second.description = "second".into();
    req.tools = vec![tool("read"), second];
    let tools = body(&req)["tools"].as_array().unwrap().clone();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["function"]["description"], "second");
}

#[test]
fn endpoint_query_parameters_survive_the_protocol_path() {
    let mut req = request(Protocol::ChatCompletions, Supports::default());
    req.messages = vec![Message::user_text("hi")];
    let endpoint = systemprompt_engine::llm::Endpoint::new(
        Url::parse("https://gateway.test/v1?api-version=2026-01&tenant=t1").unwrap(),
    );
    let prepared = systemprompt_engine::llm::prepare(&endpoint, &req).unwrap();
    assert_eq!(
        prepared.url.as_str(),
        "https://gateway.test/v1/chat/completions?api-version=2026-01&tenant=t1"
    );
    req.model.protocol = Protocol::Messages;
    let prepared = systemprompt_engine::llm::prepare(&endpoint, &req).unwrap();
    assert_eq!(prepared.url.query(), Some("api-version=2026-01&tenant=t1"));
}
