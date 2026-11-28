//! The messages-protocol request body: system blocks, turns, tool calls and
//! results, media, reasoning settings, output limits and cache markers.

use serde_json::{Value, json};
use systemprompt_engine::llm::{
    Attachment, Content, Message, Protocol, Reasoning, ReasoningEffort, Supports, ToolChoice,
    ToolDef,
};

use crate::support::{all_supports, body, model, request};

fn tool(name: &str) -> ToolDef {
    ToolDef {
        name: name.into(),
        description: format!("the {name} tool"),
        input_schema: json!({"type": "object", "properties": {}}),
    }
}

fn markers(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            usize::from(map.contains_key("cache_control"))
                + map.values().map(markers).sum::<usize>()
        },
        Value::Array(items) => items.iter().map(markers).sum(),
        _ => 0,
    }
}

#[test]
fn empty_system_text_is_omitted_and_other_text_keeps_its_whitespace() {
    let mut req = request(Protocol::Messages, Supports::default());
    req.system = vec![String::new(), "   ".into(), "  be brief \n".into()];
    req.messages = vec![Message::user_text("hi")];
    assert_eq!(
        body(&req)["system"],
        json!([{"type": "text", "text": "  be brief \n"}])
    );
    req.system = vec![" ".into()];
    assert!(body(&req).get("system").is_none());
}

#[test]
fn whitespace_only_text_is_dropped_and_a_message_left_empty_is_removed() {
    let mut req = request(Protocol::Messages, Supports::default());
    req.messages = vec![
        Message::user_text("question"),
        Message::assistant(vec![Content::text("  \n ")]),
        Message::user(vec![Content::text("more"), Content::text("\t")]),
    ];
    let messages = body(&req)["messages"].clone();
    assert_eq!(messages.as_array().unwrap().len(), 1);
    assert_eq!(
        messages[0]["content"],
        json!([{"type": "text", "text": "question"}, {"type": "text", "text": "more"}])
    );
}

#[test]
fn an_effort_becomes_adaptive_thinking_with_that_effort() {
    let mut req = request(Protocol::Messages, all_supports());
    req.params.reasoning = Some(Reasoning::Effort(ReasoningEffort::High));
    let shaped = body(&req);
    assert_eq!(shaped["thinking"], json!({"type": "adaptive"}));
    assert_eq!(shaped["output_config"], json!({"effort": "high"}));
}

#[test]
fn a_budget_becomes_enabled_thinking_and_no_reasoning_sends_none() {
    let mut req = request(Protocol::Messages, all_supports());
    req.params.reasoning = Some(Reasoning::Budget(2_000));
    assert_eq!(
        body(&req)["thinking"],
        json!({"type": "enabled", "budget_tokens": 2_000})
    );
    req.params.reasoning = None;
    assert!(body(&req).get("thinking").is_none());
}

#[test]
#[ignore = "parity ledger: the thinking budget is capped one below the output limit, not at half of it"]
fn the_thinking_budget_is_fitted_to_half_the_output_limit() {
    let mut req = request(Protocol::Messages, all_supports());
    req.params.max_output = Some(8_000);
    req.params.reasoning = Some(Reasoning::Budget(20_000));
    assert_eq!(body(&req)["thinking"]["budget_tokens"], 4_000);
}

#[test]
fn thinking_is_spelled_the_same_for_a_model_version_not_yet_released() {
    let mut req = request(Protocol::Messages, all_supports());
    req.model.id = "claude-future-9-9".into();
    req.params.reasoning = Some(Reasoning::Effort(ReasoningEffort::Low));
    let shaped = body(&req);
    assert_eq!(shaped["thinking"], json!({"type": "adaptive"}));
    assert_eq!(shaped["output_config"]["effort"], "low");
}

#[test]
fn tool_calls_and_results_become_tool_use_and_tool_result_blocks() {
    let mut req = request(
        Protocol::Messages,
        Supports {
            caching: false,
            ..all_supports()
        },
    );
    req.tools = vec![tool("read")];
    req.messages = vec![
        Message::user_text("read it"),
        Message::assistant(vec![Content::ToolCall {
            id: "t1".into(),
            name: "read".into(),
            input: json!({"path": "a"}),
        }]),
        Message::user(vec![Content::ToolResult {
            id: "t1".into(),
            output: "contents".into(),
            is_error: true,
            attachments: vec![],
        }]),
    ];
    let shaped = body(&req);
    assert_eq!(
        shaped["messages"][1]["content"][0],
        json!({"type": "tool_use", "id": "t1", "name": "read", "input": {"path": "a"}})
    );
    assert_eq!(
        shaped["messages"][2]["content"][0],
        json!({"type": "tool_result", "tool_use_id": "t1", "content": "contents", "is_error": true})
    );
}

#[test]
fn tool_call_ids_are_scrubbed_of_characters_the_protocol_refuses_but_not_shortened() {
    let long = format!("call.{}:end", "x".repeat(80));
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("go"),
        Message::assistant(vec![Content::ToolCall {
            id: long.clone(),
            name: "read".into(),
            input: json!({}),
        }]),
        Message::user(vec![Content::ToolResult {
            id: long.clone(),
            output: "ok".into(),
            is_error: false,
            attachments: vec![],
        }]),
    ];
    let shaped = body(&req);
    let expected = format!("call_{}_end", "x".repeat(80));
    assert_eq!(shaped["messages"][1]["content"][0]["id"], expected.as_str());
    assert_eq!(
        shaped["messages"][2]["content"][0]["tool_use_id"],
        expected.as_str()
    );
}

#[test]
fn parallel_tool_results_go_back_in_one_user_turn() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("go"),
        Message::assistant(vec![
            Content::ToolCall {
                id: "a".into(),
                name: "read".into(),
                input: json!({}),
            },
            Content::ToolCall {
                id: "b".into(),
                name: "read".into(),
                input: json!({}),
            },
        ]),
        Message::user(vec![Content::ToolResult {
            id: "a".into(),
            output: "1".into(),
            is_error: false,
            attachments: vec![],
        }]),
        Message::user(vec![Content::ToolResult {
            id: "b".into(),
            output: "2".into(),
            is_error: false,
            attachments: vec![],
        }]),
    ];
    let messages = body(&req)["messages"].as_array().unwrap().clone();
    assert_eq!(messages.len(), 3);
    let ids: Vec<&str> = messages[2]["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["tool_use_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn tools_stay_declared_when_the_choice_is_none() {
    let mut req = request(Protocol::Messages, all_supports());
    req.tools = vec![tool("read")];
    req.params.tool_choice = ToolChoice::None;
    let shaped = body(&req);
    assert_eq!(shaped["tools"][0]["name"], "read");
    assert_eq!(shaped["tool_choice"], json!({"type": "none"}));
}

#[test]
fn media_in_a_tool_result_is_sent_as_structured_blocks() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("look"),
        Message::assistant(vec![Content::ToolCall {
            id: "t".into(),
            name: "shot".into(),
            input: json!({}),
        }]),
        Message::user(vec![Content::ToolResult {
            id: "t".into(),
            output: "a screenshot and a report".into(),
            is_error: false,
            attachments: vec![
                Attachment {
                    mime: "image/png".into(),
                    data: "iVBO".into(),
                    name: None,
                },
                Attachment {
                    mime: "application/pdf".into(),
                    data: "JVBE".into(),
                    name: Some("r.pdf".into()),
                },
            ],
        }]),
    ];
    let content = body(&req)["messages"][2]["content"][0]["content"].clone();
    assert_eq!(
        content[0],
        json!({"type": "text", "text": "a screenshot and a report"})
    );
    assert_eq!(
        content[1],
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBO"}})
    );
    assert_eq!(content[2]["type"], "document");
    assert_eq!(content[2]["source"]["media_type"], "application/pdf");
}

#[test]
fn a_tool_result_with_one_image_and_no_text_is_a_single_image_block() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![Message::user(vec![Content::ToolResult {
        id: "t".into(),
        output: String::new(),
        is_error: false,
        attachments: vec![Attachment {
            mime: "image/jpeg".into(),
            data: "/9j/".into(),
            name: None,
        }],
    }])];
    let content = body(&req)["messages"][0]["content"][0]["content"].clone();
    assert_eq!(content.as_array().unwrap().len(), 1);
    assert_eq!(content[0]["type"], "image");
}

#[test]
#[ignore = "parity ledger: media a tool result cannot carry is replaced by a visible notice rather than refused"]
fn tool_result_media_that_cannot_be_sent_is_refused() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![Message::user(vec![Content::ToolResult {
        id: "t".into(),
        output: String::new(),
        is_error: false,
        attachments: vec![Attachment {
            mime: "video/mp4".into(),
            data: "AAAA".into(),
            name: None,
        }],
    }])];
    let content = body(&req)["messages"][0]["content"][0]["content"].clone();
    assert!(content.to_string().contains("video/mp4") && !content.to_string().contains("omitted"));
}

#[test]
fn signed_reasoning_goes_back_with_its_signature_and_unsigned_reasoning_is_dropped() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("q"),
        Message::assistant(vec![
            Content::Reasoning {
                text: "kept".into(),
                signature: Some("s1".into()),
            },
            Content::Reasoning {
                text: "unsigned".into(),
                signature: None,
            },
            Content::text("answer"),
        ]),
    ];
    let content = body(&req)["messages"][1]["content"].clone();
    assert_eq!(
        content[0],
        json!({"type": "thinking", "thinking": "kept", "signature": "s1"})
    );
    assert_eq!(content.as_array().unwrap().len(), 2);
    assert!(!content.to_string().contains("unsigned"));
}

#[test]
fn a_user_image_continues_the_conversation_as_an_image_block() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![Message::user(vec![
        Content::text("what is this"),
        Content::Image {
            mime: "image/png".into(),
            data: "iVBO".into(),
        },
    ])];
    let content = body(&req)["messages"][0]["content"].clone();
    assert_eq!(content[1]["source"]["media_type"], "image/png");
}

#[test]
fn a_user_pdf_reaches_the_model_as_a_document() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![Message::user(vec![
        Content::text("summarise"),
        Content::Document {
            mime: "application/pdf".into(),
            data: "JVBERi0=".into(),
            name: Some("spec.pdf".into()),
        },
    ])];
    let block = body(&req)["messages"][0]["content"][1].clone();
    assert_eq!(block["type"], "document");
    assert_eq!(
        block["source"],
        json!({"type": "base64", "media_type": "application/pdf", "data": "JVBERi0="})
    );
    assert_eq!(block["title"], "spec.pdf");
}

#[test]
fn the_output_limit_defaults_to_the_protocol_default_when_nothing_states_one() {
    let mut req = request(Protocol::Messages, Supports::default());
    req.model.max_output = 0;
    req.messages = vec![Message::user_text("hi")];
    assert_eq!(body(&req)["max_tokens"], 8_192);
    req.params.max_output = Some(100);
    assert_eq!(body(&req)["max_tokens"], 100);
}

#[test]
fn with_caching_the_system_prompt_and_the_conversation_tail_carry_markers() {
    let mut req = request(Protocol::Messages, all_supports());
    req.system = vec!["first".into(), "last".into()];
    req.messages = vec![
        Message::user_text("one"),
        Message::assistant(vec![Content::text("two")]),
        Message::user_text("three"),
    ];
    let shaped = body(&req);
    assert!(shaped["system"][1].get("cache_control").is_some());
    assert!(
        shaped["messages"][1]["content"][0]
            .get("cache_control")
            .is_some()
    );
    assert!(
        shaped["messages"][2]["content"][0]
            .get("cache_control")
            .is_some()
    );
    assert!(
        shaped["messages"][0]["content"][0]
            .get("cache_control")
            .is_none()
    );
}

#[test]
#[ignore = "parity ledger: the tool list and the first system block carry no cache marker"]
fn with_caching_the_last_tool_and_the_first_system_block_carry_markers() {
    let mut req = request(Protocol::Messages, all_supports());
    req.tools = vec![tool("read"), tool("write")];
    req.system = vec!["first".into(), "last".into()];
    req.messages = vec![Message::user_text("hi")];
    let shaped = body(&req);
    assert!(shaped["tools"][1].get("cache_control").is_some());
    assert!(shaped["system"][0].get("cache_control").is_some());
}

#[test]
#[ignore = "parity ledger: a tool result block in the tail carries the marker, but tool definitions never do"]
fn tool_definitions_and_tool_results_carry_cache_markers() {
    let mut req = request(Protocol::Messages, all_supports());
    req.tools = vec![tool("read")];
    req.messages = vec![Message::user(vec![Content::ToolResult {
        id: "t".into(),
        output: "x".into(),
        is_error: false,
        attachments: vec![],
    }])];
    let shaped = body(&req);
    assert!(
        shaped["messages"][0]["content"][0]
            .get("cache_control")
            .is_some()
    );
    assert!(shaped["tools"][0].get("cache_control").is_some());
}

#[test]
fn a_request_never_carries_more_than_four_cache_markers() {
    let mut req = request(Protocol::Messages, all_supports());
    req.tools = (0..6).map(|n| tool(&format!("t{n}"))).collect();
    req.system = (0..5).map(|n| format!("system {n}")).collect();
    req.messages = (0..12)
        .map(|n| {
            if n % 2 == 0 {
                Message::user_text(format!("u{n}"))
            } else {
                Message::assistant(vec![Content::text(format!("a{n}"))])
            }
        })
        .collect();
    assert!(markers(&body(&req)) <= 4);
}

#[test]
fn caching_marks_the_last_two_turns_and_skips_reasoning_blocks() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("one"),
        Message::assistant(vec![
            Content::text("two"),
            Content::Reasoning {
                text: "r".into(),
                signature: Some("s".into()),
            },
        ]),
        Message::user_text("three"),
    ];
    let shaped = body(&req);
    let assistant = &shaped["messages"][1]["content"];
    assert!(assistant[0].get("cache_control").is_some());
    assert!(assistant[1].get("cache_control").is_none());
    assert_eq!(markers(&shaped["messages"]), 2);
}

#[test]
fn a_reasoning_only_assistant_turn_gets_no_text_cache_marker() {
    let mut req = request(Protocol::Messages, all_supports());
    req.messages = vec![
        Message::user_text("one"),
        Message::assistant(vec![Content::Reasoning {
            text: "r".into(),
            signature: Some("s".into()),
        }]),
    ];
    let shaped = body(&req);
    assert_eq!(markers(&shaped["messages"][1]), 0);
}

#[test]
fn without_caching_support_no_markers_are_sent() {
    let mut req = request(
        Protocol::Messages,
        Supports {
            caching: false,
            ..all_supports()
        },
    );
    req.system = vec!["s".into()];
    req.tools = vec![tool("read")];
    req.messages = vec![Message::user_text("a")];
    assert_eq!(markers(&body(&req)), 0);
}

#[test]
fn a_long_tool_turn_keeps_a_marker_on_its_last_block() {
    let mut req = request(Protocol::Messages, all_supports());
    let calls: Vec<Content> = (0..30)
        .map(|n| Content::ToolCall {
            id: format!("c{n}"),
            name: "read".into(),
            input: json!({}),
        })
        .collect();
    let results: Vec<Content> = (0..30)
        .map(|n| Content::ToolResult {
            id: format!("c{n}"),
            output: format!("r{n}"),
            is_error: false,
            attachments: vec![],
        })
        .collect();
    req.messages = vec![
        Message::user_text("go"),
        Message::assistant(calls),
        Message::user(results),
    ];
    let shaped = body(&req);
    let last = shaped["messages"][2]["content"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert!(last.get("cache_control").is_some());
}

#[test]
fn an_image_the_model_cannot_read_is_replaced_by_a_visible_notice() {
    let mut req = request(
        Protocol::Messages,
        Supports {
            images: false,
            ..Supports::default()
        },
    );
    req.messages = vec![
        Message::user_text("plain"),
        Message::assistant(vec![Content::text("ok")]),
        Message::user(vec![Content::Image {
            mime: "image/png".into(),
            data: "iVBO".into(),
        }]),
    ];
    let shaped = body(&req);
    let notice = shaped["messages"][2]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        notice.contains("image/png") && notice.contains("omitted"),
        "{notice}"
    );
    assert_eq!(
        shaped["messages"][0]["content"],
        json!([{"type": "text", "text": "plain"}])
    );
}

#[test]
fn an_image_inside_a_tool_result_the_model_cannot_read_is_replaced_by_a_notice() {
    let mut req = request(
        Protocol::Messages,
        Supports {
            images: false,
            ..Supports::default()
        },
    );
    req.messages = vec![Message::user(vec![Content::ToolResult {
        id: "t".into(),
        output: "see".into(),
        is_error: false,
        attachments: vec![Attachment {
            mime: "image/webp".into(),
            data: "UklG".into(),
            name: None,
        }],
    }])];
    let content = body(&req)["messages"][0]["content"][0]["content"].clone();
    assert!(content[1]["text"].as_str().unwrap().contains("omitted"));
}

#[test]
fn images_a_model_accepts_are_sent_unchanged() {
    let mut req = request(
        Protocol::Messages,
        Supports {
            images: true,
            ..Supports::default()
        },
    );
    req.messages = vec![Message::user(vec![Content::Image {
        mime: "image/png".into(),
        data: "iVBO".into(),
    }])];
    assert_eq!(
        body(&req)["messages"][0]["content"][0]["source"]["data"],
        "iVBO"
    );
}

#[test]
fn the_same_spelling_is_used_whatever_the_model_is_named() {
    let base = model(Protocol::Messages, all_supports());
    let mut one = request(Protocol::Messages, all_supports());
    one.params.reasoning = Some(Reasoning::Budget(4_000));
    let mut two = one.clone();
    two.model = systemprompt_engine::llm::ModelSpec {
        id: "another-vendor-model".into(),
        ..base
    };
    assert_eq!(body(&one)["thinking"], body(&two)["thinking"]);
}
