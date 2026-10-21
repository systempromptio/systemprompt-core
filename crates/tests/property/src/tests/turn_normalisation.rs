// Why: strict Anthropic-compatible upstreams (DeepSeek, MiniMax), OpenAI and
// Gemini each reject a history whose roles repeat or whose tool calls are not
// answered by the very next turn. Request ba6f9208 was one such history, built
// from parallel OpenCode tool messages. These state, for any history a client
// can send, what the normaliser and every upstream encoder must guarantee.

use std::collections::BTreeSet;

use proptest::prelude::*;
use serde_json::Value;
use systemprompt_identifiers::ModelId;
use systemprompt_wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, INTERRUPTED_TOOL_RESULT, Role, TurnMerge,
    normalize_turns,
};
use systemprompt_wire::{anthropic, gemini, openai_chat, openai_responses};

use crate::strategies::canonical::{arb_history, arb_valid_history};

const MERGES: [TurnMerge; 2] = [TurnMerge::UserSide, TurnMerge::UserAndTool];

fn part_key(part: &CanonicalContent) -> String {
    match part {
        CanonicalContent::Text { text, .. } => format!("text:{text}"),
        CanonicalContent::Image { .. } => "image".to_owned(),
        CanonicalContent::Thinking { text, .. } => format!("thinking:{text}"),
        CanonicalContent::ToolUse { id, .. } => format!("use:{id}"),
        CanonicalContent::ToolResult {
            tool_use_id,
            is_error,
            ..
        } => format!("result:{tool_use_id}:{is_error}"),
        CanonicalContent::AnthropicToolBlock { tool_name, .. } => format!("server:{tool_name}"),
    }
}

fn shape(messages: &[CanonicalMessage]) -> Vec<(Role, Vec<String>)> {
    messages
        .iter()
        .map(|m| (m.role, m.content.iter().map(part_key).collect()))
        .collect()
}

fn is_assistant(m: &CanonicalMessage) -> bool {
    m.role == Role::Assistant
}

fn uses(m: &CanonicalMessage) -> Vec<&str> {
    m.content
        .iter()
        .filter_map(|p| match p {
            CanonicalContent::ToolUse { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

fn results(m: &CanonicalMessage) -> Vec<&str> {
    m.content
        .iter()
        .filter_map(|p| match p {
            CanonicalContent::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
            _ => None,
        })
        .collect()
}

fn synthetic(part: &CanonicalContent) -> bool {
    matches!(part, CanonicalContent::ToolResult { is_error: true, content, .. }
        if matches!(content.first(), Some(CanonicalContent::Text { text, .. }) if text == INTERRUPTED_TOOL_RESULT))
}

fn request(messages: Vec<CanonicalMessage>) -> CanonicalRequest {
    CanonicalRequest::new(ModelId::new("m"), messages, 64)
}

fn array(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}

fn ids<'a>(blocks: &'a [Value], kind: &str, key: &str) -> Vec<&'a str> {
    blocks
        .iter()
        .filter(|b| b["type"] == kind)
        .filter_map(|b| b[key].as_str())
        .collect()
}

// Messages alternate, and each assistant turn's tool_use ids are answered
// exactly once by the next turn and nowhere else.
fn anthropic_is_strict(body: &Value) -> Result<(), String> {
    let messages = array(&body["messages"]);
    let mut open: Vec<&str> = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        if i > 0 && messages[i - 1]["role"] == m["role"] {
            return Err(format!("messages.{i} repeats role {}", m["role"]));
        }
        let content = array(&m["content"]);
        let mut answered = ids(content, "tool_result", "tool_use_id");
        if m["role"] == "user" {
            answered.sort_unstable();
            open.sort_unstable();
            if answered != open {
                return Err(format!("messages.{i} answers {answered:?}, open {open:?}"));
            }
            open.clear();
        } else {
            if !open.is_empty() || !answered.is_empty() {
                return Err(format!("messages.{i} leaves {open:?} unanswered"));
            }
            open = ids(content, "tool_use", "id");
        }
    }
    if open.is_empty() {
        Ok(())
    } else {
        Err(format!("{open:?} never answered"))
    }
}

fn openai_chat_is_strict(body: &Value) -> Result<(), String> {
    let mut open: BTreeSet<&str> = BTreeSet::new();
    let mut previous = "";
    for (i, m) in array(&body["messages"]).iter().enumerate() {
        let role = m["role"].as_str().unwrap_or_default();
        if role == "tool" {
            if !open.remove(m["tool_call_id"].as_str().unwrap_or_default()) {
                return Err(format!("messages.{i} answers no open call"));
            }
        } else {
            if !open.is_empty() {
                return Err(format!("messages.{i} before {open:?} answered"));
            }
            if role == "assistant" {
                if previous == "assistant" {
                    return Err(format!("messages.{i} repeats assistant"));
                }
                open = array(&m["tool_calls"])
                    .iter()
                    .filter_map(|c| c["id"].as_str())
                    .collect();
            }
        }
        previous = role;
    }
    if open.is_empty() {
        Ok(())
    } else {
        Err(format!("{open:?} never answered"))
    }
}

fn openai_responses_is_strict(body: &Value) -> Result<(), String> {
    let mut open: BTreeSet<&str> = BTreeSet::new();
    for (i, item) in array(&body["input"]).iter().enumerate() {
        let call = item["call_id"].as_str().unwrap_or_default();
        match item["type"].as_str() {
            Some("function_call") => {
                open.insert(call);
            },
            Some("function_call_output") if !open.remove(call) => {
                return Err(format!("input.{i} answers no open call"));
            },
            Some("message") if !open.is_empty() => {
                return Err(format!("input.{i} before {open:?} answered"));
            },
            _ => {},
        }
    }
    if open.is_empty() {
        Ok(())
    } else {
        Err(format!("{open:?} never answered"))
    }
}

fn gemini_is_strict(body: &Value) -> Result<(), String> {
    let contents = array(&body["contents"]);
    let mut open = 0usize;
    for (i, c) in contents.iter().enumerate() {
        if i > 0 && contents[i - 1]["role"] == c["role"] {
            return Err(format!("contents.{i} repeats role {}", c["role"]));
        }
        let parts = array(&c["parts"]);
        let count = |key: &str| parts.iter().filter(|p| p.get(key).is_some()).count();
        if c["role"] == "user" {
            if count("functionResponse") != open {
                return Err(format!(
                    "contents.{i} answers {} of {open}",
                    count("functionResponse")
                ));
            }
            open = 0;
        } else {
            open = count("functionCall");
        }
    }
    if open == 0 {
        Ok(())
    } else {
        Err(format!("{open} calls never answered"))
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    // Sides alternate and every call is answered exactly once by the next
    // turn, whatever the client sent.
    #[test]
    fn normalised_turns_alternate_and_pair_every_call(history in arb_history()) {
        for merge in MERGES {
            let out = normalize_turns(&history, merge);
            for (i, pair) in out.windows(2).enumerate() {
                let system = |m: &CanonicalMessage| merge == TurnMerge::UserAndTool && m.role == Role::System;
                if !system(&pair[0]) && !system(&pair[1]) {
                    prop_assert_ne!(is_assistant(&pair[0]), is_assistant(&pair[1]), "turn {} repeats a side: {:?}", i, shape(&out));
                }
                if is_assistant(&pair[0]) && !uses(&pair[0]).is_empty() {
                    let mut asked = uses(&pair[0]);
                    let mut answered = results(&pair[1]);
                    asked.sort_unstable();
                    answered.sort_unstable();
                    prop_assert_eq!(asked, answered, "{:?}", shape(&out));
                }
            }
            if let Some(last) = out.last() {
                prop_assert!(!is_assistant(last) || uses(last).is_empty());
            }
        }
    }

    #[test]
    fn normalisation_is_idempotent(history in arb_history()) {
        for merge in MERGES {
            let once = normalize_turns(&history, merge);
            let twice = normalize_turns(&once, merge);
            prop_assert_eq!(shape(&once), shape(&twice));
        }
    }

    // The production-replay guarantee: traffic that was already valid goes
    // upstream exactly as the client laid it out.
    #[test]
    fn already_valid_histories_are_unchanged(history in arb_valid_history()) {
        for merge in MERGES {
            prop_assert_eq!(shape(&normalize_turns(&history, merge)), shape(&history));
        }
    }

    // Nothing but a stray or duplicate result is ever dropped, and nothing is
    // added but an interrupted-call result.
    #[test]
    fn no_content_is_lost_except_orphaned_results(history in arb_history()) {
        for merge in MERGES {
            let out = normalize_turns(&history, merge);
            let kept = |messages: &[CanonicalMessage]| -> Vec<String> {
                let mut keys: Vec<String> = messages
                    .iter()
                    .flat_map(|m| m.content.iter())
                    .filter(|p| !matches!(p, CanonicalContent::ToolResult { .. }))
                    .map(part_key)
                    .collect();
                keys.sort();
                keys
            };
            prop_assert_eq!(kept(&history), kept(&out));
            let sent: BTreeSet<String> = history.iter().flat_map(|m| m.content.iter()).map(part_key).collect();
            for part in out.iter().flat_map(|m| m.content.iter()) {
                prop_assert!(sent.contains(&part_key(part)) || synthetic(part), "invented {}", part_key(part));
            }
        }
    }

    #[test]
    fn every_upstream_encoder_emits_a_strict_history(history in arb_history()) {
        let req = request(history);
        let checks: [(&str, Result<(), String>); 4] = [
            ("anthropic", anthropic_is_strict(&anthropic::build_request_body(&req, "m", None))),
            ("openai_chat", openai_chat_is_strict(&openai_chat::build_request_body(&req, "m", None))),
            ("openai_responses", openai_responses_is_strict(&openai_responses::build_request_body(&req, "m", None))),
            ("gemini", gemini_is_strict(&gemini::build_request_body(&req, None))),
        ];
        for (wire, check) in checks {
            prop_assert!(check.is_ok(), "{}: {:?}", wire, check);
        }
    }
}
