use serde_json::{Value, json};
use systemprompt_engine::llm::{
    Content, Message, ModelSpec, Params, Protocol, Reasoning, ReasoningEffort, Request, Supports,
    ToolDef, request_body,
};

fn spec() -> ModelSpec {
    ModelSpec {
        id: "claude-test".into(),
        name: "Claude Test".into(),
        protocol: Protocol::Messages,
        context_window: 200_000,
        max_output: 32_000,
        supports: Supports {
            images: true,
            reasoning: true,
            tool_calls: true,
            caching: true,
        },
    }
}

fn body(request: &Request) -> Value {
    request_body(request, None)
}

fn sample_request() -> Request {
    let mut request = Request::new(spec());
    request.system = vec!["You are a coding agent.".into(), "Project rules.".into()];
    request.tools = vec![ToolDef {
        name: "read".into(),
        description: "Read a file.".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
    }];
    request.messages = vec![
        Message::user_text("Open main.rs"),
        Message::assistant(vec![
            Content::Reasoning {
                text: "Need the file.".into(),
                signature: Some("sig-abc".into()),
            },
            Content::Reasoning {
                text: "unsigned".into(),
                signature: None,
            },
            Content::text("Reading it.  "),
            Content::ToolCall {
                id: "call:1".into(),
                name: "read".into(),
                input: json!({"path": "src/main.rs"}),
            },
        ]),
        Message::user(vec![
            Content::text("also check tests"),
            Content::ToolResult {
                id: "call:1".into(),
                output: "fn main() {}".into(),
                is_error: false,
                attachments: vec![],
            },
        ]),
    ];
    request.params = Params {
        max_output: Some(4_096),
        temperature: Some(0.2),
        ..Params::default()
    };
    request.metadata.insert("user_id".into(), json!("u-1"));
    request
}

#[test]
fn body_is_shaped_for_messages() {
    let body = body(&sample_request());
    let expected = json!({
        "model": "claude-test",
        "system": [
            {"type": "text", "text": "You are a coding agent."},
            {"type": "text", "text": "Project rules.", "cache_control": {"type": "ephemeral"}}
        ],
        "messages": [
            {"role": "user", "content": [{"type": "text", "text": "Open main.rs"}]},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "Need the file.", "signature": "sig-abc"},
                {"type": "text", "text": "Reading it.  "},
                {"type": "tool_use", "id": "call_1", "name": "read", "input": {"path": "src/main.rs"},
                 "cache_control": {"type": "ephemeral"}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "call_1", "content": "fn main() {}"},
                {"type": "text", "text": "also check tests", "cache_control": {"type": "ephemeral"}}
            ]}
        ],
        "tools": [{"name": "read", "description": "Read a file.",
                   "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}}}],
        "max_tokens": 4096,
        "temperature": 0.2,
        "stream": true,
        "metadata": {"user_id": "u-1"}
    });
    assert_eq!(body, expected);
}

#[test]
fn thinking_budget_drops_sampling_and_fits_max_tokens() {
    let mut request = sample_request();
    request.params.reasoning = Some(Reasoning::Budget(10_000));
    let body = body(&request);
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 4095})
    );
    assert!(body.get("temperature").is_none());

    request.params.reasoning = Some(Reasoning::Effort(ReasoningEffort::High));
    let body = self::body(&request);
    assert_eq!(body["thinking"], json!({"type": "adaptive"}));
    assert_eq!(body["output_config"], json!({"effort": "high"}));
}

#[test]
fn no_cache_markers_without_caching_support() {
    let mut request = sample_request();
    request.model.supports.caching = false;
    assert!(!body(&request).to_string().contains("cache_control"));
}

#[test]
fn final_assistant_prefill_is_trimmed_and_consecutive_turns_merge() {
    let mut request = Request::new(spec());
    request.messages = vec![
        Message::user_text("a"),
        Message::user_text("b"),
        Message::assistant(vec![Content::text("prefill  \n")]),
    ];
    request.model.supports.caching = false;
    let body = body(&request);
    assert_eq!(
        body["messages"][0]["content"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(body["messages"][1]["content"][0]["text"], json!("prefill"));
}
