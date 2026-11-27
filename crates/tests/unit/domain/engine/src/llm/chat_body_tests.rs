use serde_json::{Value, json};
use systemprompt_engine::llm::{
    Attachment, Content, Message, ModelSpec, Params, Protocol, Reasoning, Request, Supports,
    ToolChoice, ToolDef, request_body,
};

fn spec() -> ModelSpec {
    ModelSpec {
        id: "gpt-test".into(),
        name: "GPT Test".into(),
        protocol: Protocol::ChatCompletions,
        context_window: 128_000,
        max_output: 16_000,
        supports: Supports {
            images: true,
            reasoning: true,
            tool_calls: true,
            caching: false,
        },
    }
}

fn body(request: &Request) -> Value {
    request_body(request, None)
}

#[test]
fn body_is_shaped_for_chat_completions() {
    let mut request = Request::new(spec());
    request.system = vec!["You are a coding agent.".into()];
    request.tools = vec![ToolDef {
        name: "read".into(),
        description: "Read a file.".into(),
        input_schema: json!({"type": "object"}),
    }];
    request.messages = vec![
        Message::user_text("Open a.rs"),
        Message::assistant(vec![
            Content::Reasoning {
                text: "hidden".into(),
                signature: None,
            },
            Content::ToolCall {
                id: "call_a".into(),
                name: "read".into(),
                input: json!({"path": "a.rs"}),
            },
        ]),
        Message::user(vec![Content::ToolResult {
            id: "call_a".into(),
            output: "see image".into(),
            is_error: false,
            attachments: vec![Attachment {
                mime: "image/png".into(),
                data: "AAAA".into(),
                name: None,
            }],
        }]),
    ];
    request.params = Params {
        max_output: Some(50_000),
        temperature: Some(0.5),
        tool_choice: ToolChoice::Required,
        ..Params::default()
    };
    let expected = json!({
        "model": "gpt-test",
        "messages": [
            {"role": "system", "content": "You are a coding agent."},
            {"role": "user", "content": "Open a.rs"},
            {"role": "assistant", "content": null, "tool_calls": [
                {"id": "call_a", "type": "function", "function": {"name": "read", "arguments": "{\"path\":\"a.rs\"}"}}
            ]},
            {"role": "tool", "tool_call_id": "call_a", "content": "see image"},
            {"role": "user", "content": [{"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}]}
        ],
        "tools": [{"type": "function", "function": {"name": "read", "description": "Read a file.", "parameters": {"type": "object"}}}],
        "tool_choice": "required",
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_tokens": 16000,
        "temperature": 0.5
    });
    assert_eq!(body(&request), expected);
}

#[test]
fn reasoning_replaces_sampling() {
    let mut request = Request::new(spec());
    request.messages = vec![Message::user_text("hi")];
    request.params.temperature = Some(0.3);
    request.params.reasoning = Some(Reasoning::Budget(20_000));
    let body = body(&request);
    assert_eq!(body["reasoning_effort"], json!("high"));
    assert!(body.get("temperature").is_none());
}

#[test]
fn endpoint_metadata_merges_into_the_body() {
    let mut request = Request::new(spec());
    request.metadata.insert("trace".into(), json!("t"));
    let extra = json!({"metadata": {"user_id": "u"}, "service_tier": "flex"});
    let body = request_body(&request, Some(&extra));
    assert_eq!(body["metadata"], json!({"trace": "t", "user_id": "u"}));
    assert_eq!(body["service_tier"], json!("flex"));
}
