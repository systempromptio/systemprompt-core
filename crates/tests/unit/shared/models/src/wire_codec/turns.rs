//! Turn normalisation (`canonical::normalize_turns`) and the upstream shapes
//! every encoder builds from it.
//!
//! Request `ba6f9208` is the case these guard: `OpenCode` sent one
//! `role: "tool"` message per parallel call, the anthropic encoder rendered
//! each as its own user turn, and `DeepSeek`'s Anthropic-compatible endpoint
//! rejected the second `tool_use` as unanswered.

use serde_json::{Value, json};
use systemprompt_wire::canonical::{
    CanonicalContent, CanonicalMessage, INTERRUPTED_TOOL_RESULT, Role, TurnMerge, normalize_turns,
};
use systemprompt_wire::{anthropic, gemini, openai_chat, openai_responses};

use super::{assistant_calls, base_request};

fn message(role: Role, content: Vec<CanonicalContent>) -> CanonicalMessage {
    CanonicalMessage { role, content }
}

fn result(id: &str) -> CanonicalContent {
    CanonicalContent::ToolResult {
        tool_use_id: id.to_owned(),
        content: vec![CanonicalContent::text(format!("result of {id}"))],
        is_error: false,
        structured_content: None,
        meta: None,
        cache_control: None,
    }
}

fn text(t: &str) -> CanonicalContent {
    CanonicalContent::text(t.to_owned())
}

// One line per message: its role and the kind (and id) of each part.
fn shape(messages: &[CanonicalMessage]) -> Vec<String> {
    messages
        .iter()
        .map(|m| {
            let parts: Vec<String> = m
                .content
                .iter()
                .map(|part| match part {
                    CanonicalContent::Text { text, .. } => format!("text:{text}"),
                    CanonicalContent::ToolUse { id, .. } => format!("use:{id}"),
                    CanonicalContent::ToolResult {
                        tool_use_id,
                        is_error,
                        ..
                    } => format!("result:{tool_use_id}{}", if *is_error { "!" } else { "" }),
                    CanonicalContent::Thinking { text, .. } => format!("thinking:{text}"),
                    CanonicalContent::Image { .. } => "image".to_owned(),
                    CanonicalContent::AnthropicToolBlock { tool_name, .. } => {
                        format!("server:{tool_name}")
                    },
                })
                .collect();
            format!("{}[{}]", m.role.as_str(), parts.join(","))
        })
        .collect()
}

fn parallel_tool_messages() -> Vec<CanonicalMessage> {
    vec![
        message(Role::User, vec![text("read both")]),
        assistant_calls(&["call_a", "call_b"]),
        message(Role::Tool, vec![result("call_a")]),
        message(Role::Tool, vec![result("call_b")]),
    ]
}

struct Case {
    name: &'static str,
    merge: TurnMerge,
    input: Vec<CanonicalMessage>,
    expected: &'static [&'static str],
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "parallel tool results become one turn",
            merge: TurnMerge::UserSide,
            input: parallel_tool_messages(),
            expected: &[
                "user[text:read both]",
                "assistant[use:call_a,use:call_b]",
                "tool[result:call_a,result:call_b]",
            ],
        },
        Case {
            name: "tool results lead a turn that also carries user text",
            merge: TurnMerge::UserAndTool,
            input: vec![
                assistant_calls(&["call_a"]),
                message(Role::User, vec![text("also this")]),
                message(Role::Tool, vec![result("call_a")]),
            ],
            expected: &[
                "assistant[use:call_a]",
                "user[result:call_a,text:also this]",
            ],
        },
        Case {
            name: "an unanswered tool_use gets an interrupted error result",
            merge: TurnMerge::UserSide,
            input: vec![
                assistant_calls(&["call_a", "call_b"]),
                message(Role::Tool, vec![result("call_a")]),
                message(Role::User, vec![text("stop")]),
            ],
            expected: &[
                "assistant[use:call_a,use:call_b]",
                "user[result:call_a,result:call_b!,text:stop]",
            ],
        },
        Case {
            name: "a trailing tool_use is answered by a synthetic turn",
            merge: TurnMerge::UserSide,
            input: vec![
                message(Role::User, vec![text("go")]),
                assistant_calls(&["call_a"]),
            ],
            expected: &[
                "user[text:go]",
                "assistant[use:call_a]",
                "user[result:call_a!]",
            ],
        },
        Case {
            name: "a stray tool_result is dropped and the empty turn with it",
            merge: TurnMerge::UserSide,
            input: vec![
                message(Role::User, vec![text("hi")]),
                message(Role::Assistant, vec![text("hello")]),
                message(Role::Tool, vec![result("call_ghost")]),
                message(Role::Assistant, vec![text("again")]),
            ],
            expected: &["user[text:hi]", "assistant[text:hello,text:again]"],
        },
        Case {
            name: "a duplicated tool_result is kept once",
            merge: TurnMerge::UserSide,
            input: vec![
                assistant_calls(&["call_a"]),
                message(Role::Tool, vec![result("call_a")]),
                message(Role::Tool, vec![result("call_a")]),
            ],
            expected: &["assistant[use:call_a]", "tool[result:call_a]"],
        },
        Case {
            name: "a mid-history system message joins the user side on anthropic",
            merge: TurnMerge::UserSide,
            input: vec![
                message(Role::User, vec![text("q")]),
                message(Role::System, vec![text("reminder")]),
            ],
            expected: &["user[text:q,text:reminder]"],
        },
        Case {
            name: "a mid-history system message stays its own item on openai",
            merge: TurnMerge::UserAndTool,
            input: vec![
                message(Role::User, vec![text("q")]),
                message(Role::System, vec![text("reminder")]),
            ],
            expected: &["user[text:q]", "system[text:reminder]"],
        },
        Case {
            name: "consecutive assistant turns merge keeping thinking first",
            merge: TurnMerge::UserSide,
            input: vec![
                message(
                    Role::Assistant,
                    vec![CanonicalContent::Thinking {
                        text: "plan".to_owned(),
                        signature: Some("sig".to_owned()),
                        id: None,
                        encrypted_content: None,
                    }],
                ),
                message(Role::Assistant, vec![text("answer")]),
            ],
            expected: &["assistant[thinking:plan,text:answer]"],
        },
    ]
}

#[test]
fn normalize_turns_table() {
    for case in cases() {
        let got = shape(&normalize_turns(&case.input, case.merge));
        assert_eq!(got, case.expected, "{}", case.name);
    }
}

#[test]
fn normalize_turns_marks_synthetic_results_as_interrupted_errors() {
    let out = normalize_turns(&[assistant_calls(&["call_a"])], TurnMerge::UserSide);
    let CanonicalContent::ToolResult {
        content, is_error, ..
    } = &out[1].content[0]
    else {
        panic!("expected a synthetic tool result, got {:?}", out[1]);
    };
    assert!(*is_error);
    assert!(
        matches!(&content[0], CanonicalContent::Text { text, .. } if text == INTERRUPTED_TOOL_RESULT)
    );
}

#[test]
fn normalize_turns_keeps_a_result_after_a_server_tool_turn() {
    let input = vec![
        message(Role::User, vec![text("search")]),
        message(
            Role::Assistant,
            vec![CanonicalContent::AnthropicToolBlock {
                tool_name: "web_search".to_owned(),
                block: json!({"type": "server_tool_use", "id": "srvtoolu_1", "name": "web_search"}),
            }],
        ),
        message(Role::User, vec![result("toolu_lookup")]),
    ];
    let got = shape(&normalize_turns(&input, TurnMerge::UserSide));
    assert_eq!(got[2], "user[result:toolu_lookup]");
}

#[test]
fn normalize_turns_preserves_images_and_signatures() {
    let input = vec![
        message(
            Role::User,
            vec![super::image_url("https://example.com/a.png"), text("look")],
        ),
        message(
            Role::Assistant,
            vec![
                CanonicalContent::Thinking {
                    text: "t".to_owned(),
                    signature: Some("sig_keep".to_owned()),
                    id: None,
                    encrypted_content: None,
                },
                super::tool_use(Some("tsig_keep")),
            ],
        ),
        message(Role::Tool, vec![result("call_1")]),
    ];
    let out = normalize_turns(&input, TurnMerge::UserSide);
    assert_eq!(shape(&out)[0], "user[image,text:look]");
    assert!(
        matches!(&out[1].content[0], CanonicalContent::Thinking { signature: Some(s), .. } if s == "sig_keep")
    );
    assert!(
        matches!(&out[1].content[1], CanonicalContent::ToolUse { signature: Some(s), .. } if s == "tsig_keep")
    );
}

fn roles(messages: &Value) -> Vec<&str> {
    messages
        .as_array()
        .expect("array")
        .iter()
        .map(|m| m["role"].as_str().unwrap_or_default())
        .collect()
}

#[test]
fn anthropic_encoder_sends_parallel_results_as_one_turn() {
    let mut req = base_request();
    req.messages = parallel_tool_messages();
    let body = anthropic::build_request_body(&req, "deepseek-flash", None);
    assert_eq!(roles(&body["messages"]), ["user", "assistant", "user"]);
    let ids: Vec<&str> = body["messages"][2]["content"]
        .as_array()
        .expect("content")
        .iter()
        .filter_map(|b| b["tool_use_id"].as_str())
        .collect();
    assert_eq!(ids, ["call_a", "call_b"]);
}

#[test]
fn openai_chat_encoder_answers_both_calls_before_the_next_turn() {
    let mut req = base_request();
    req.messages = parallel_tool_messages();
    req.messages.push(message(Role::User, vec![text("next")]));
    let body = openai_chat::build_request_body(&req, "go-kimi-k3", None);
    assert_eq!(
        roles(&body["messages"]),
        ["user", "assistant", "tool", "tool", "user"]
    );
}

#[test]
fn gemini_encoder_answers_both_calls_in_one_user_turn() {
    let mut req = base_request();
    req.messages = parallel_tool_messages();
    let body = gemini::build_request_body(&req, None);
    assert_eq!(roles(&body["contents"]), ["user", "model", "user"]);
    assert_eq!(
        body["contents"][2]["parts"]
            .as_array()
            .expect("parts")
            .len(),
        2
    );
}

#[test]
fn openai_responses_encoder_puts_the_message_before_its_calls() {
    let mut req = base_request();
    let mut assistant = assistant_calls(&["call_a"]);
    assistant.content.insert(0, text("checking"));
    req.messages = vec![
        message(Role::User, vec![text("q")]),
        assistant,
        message(Role::Tool, vec![result("call_a")]),
    ];
    let body = openai_responses::build_request_body(&req, "gpt-5", None);
    let kinds: Vec<&str> = body["input"]
        .as_array()
        .expect("input")
        .iter()
        .map(|i| i["type"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        kinds,
        [
            "message",
            "message",
            "function_call",
            "function_call_output"
        ]
    );
}

#[test]
fn anthropic_history_is_strict_only_for_paired_alternating_turns() {
    let strict = json!([
        {"role": "user", "content": "q"},
        {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "n", "input": {}}]},
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "r"}]}
    ]);
    let split = json!([
        {"role": "user", "content": "q"},
        {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t1", "name": "n", "input": {}},
            {"type": "tool_use", "id": "t2", "name": "n", "input": {}}
        ]},
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "r"}]},
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t2", "content": "r"}]}
    ]);
    assert!(anthropic::history_is_strict(&strict));
    assert!(!anthropic::history_is_strict(&split));
}

#[test]
fn openai_chat_history_is_strict_only_when_every_call_is_answered_in_place() {
    let strict = json!([
        {"role": "assistant", "tool_calls": [{"id": "c1"}, {"id": "c2"}]},
        {"role": "tool", "tool_call_id": "c1", "content": "a"},
        {"role": "tool", "tool_call_id": "c2", "content": "b"},
        {"role": "user", "content": "next"}
    ]);
    let interrupted = json!([
        {"role": "assistant", "tool_calls": [{"id": "c1"}]},
        {"role": "user", "content": "stop"}
    ]);
    let stray = json!([{"role": "tool", "tool_call_id": "ghost", "content": "x"}]);
    assert!(openai_chat::history_is_strict(&strict));
    assert!(!openai_chat::history_is_strict(&interrupted));
    assert!(!openai_chat::history_is_strict(&stray));
}
