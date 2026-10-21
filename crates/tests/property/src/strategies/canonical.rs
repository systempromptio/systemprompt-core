//! Canonical conversation histories, as clients actually lay them out and as
//! no client should: parallel tool calls answered by one turn or by one
//! message each, interrupted calls, stray results, repeated roles, signed and
//! unsigned thinking, images and mid-history system messages.

use proptest::prelude::*;
use serde_json::json;
use systemprompt_wire::canonical::{CanonicalContent, CanonicalMessage, ImageSource, Role};

// One generated turn: a role selector and the part selectors inside it.
type Turn = (u8, Vec<u8>);

fn text(t: String) -> CanonicalContent {
    CanonicalContent::text(t)
}

fn image() -> CanonicalContent {
    CanonicalContent::image(ImageSource::Url {
        url: "https://example.com/i.png".to_owned(),
        detail: None,
    })
}

fn thinking(signed: bool, n: usize) -> CanonicalContent {
    CanonicalContent::Thinking {
        text: format!("thought {n}"),
        signature: signed.then(|| format!("sig_{n}")),
        id: None,
        encrypted_content: None,
    }
}

pub fn tool_use(id: String) -> CanonicalContent {
    CanonicalContent::ToolUse {
        id,
        name: "read".to_owned(),
        input: json!({ "filePath": "a.rs" }),
        signature: None,
        cache_control: None,
    }
}

pub fn tool_result(id: String) -> CanonicalContent {
    CanonicalContent::ToolResult {
        tool_use_id: id.clone(),
        content: vec![CanonicalContent::text(format!("result of {id}"))],
        is_error: false,
        structured_content: None,
        meta: None,
        cache_control: None,
    }
}

fn message(role: Role, content: Vec<CanonicalContent>) -> CanonicalMessage {
    CanonicalMessage { role, content }
}

// A result points at an issued call most of the time and at a call that never
// existed otherwise.
fn answer(issued: &[String], pick: u8, n: usize) -> String {
    if issued.is_empty() || pick.is_multiple_of(5) {
        format!("ghost_{n}")
    } else {
        issued[usize::from(pick) % issued.len()].clone()
    }
}

fn build(turns: Vec<Turn>) -> Vec<CanonicalMessage> {
    let mut issued: Vec<String> = Vec::new();
    let mut n = 0usize;
    let mut out = Vec::new();
    for (role, parts) in turns {
        let mut content = Vec::new();
        for pick in parts {
            n += 1;
            content.push(match role % 4 {
                0 => match pick % 3 {
                    0 => text(format!("user {n}")),
                    1 => image(),
                    _ => tool_result(answer(&issued, pick, n)),
                },
                1 => match pick % 4 {
                    0 => text(format!("assistant {n}")),
                    1 => thinking(true, n),
                    2 => thinking(false, n),
                    _ => {
                        let id = format!("call_{n}");
                        issued.push(id.clone());
                        tool_use(id)
                    },
                },
                2 => tool_result(answer(&issued, pick, n)),
                _ => text(format!("reminder {n}")),
            });
        }
        let role = match role % 4 {
            0 => Role::User,
            1 => Role::Assistant,
            2 => Role::Tool,
            _ => Role::System,
        };
        out.push(message(role, content));
    }
    out
}

/// Any history at all, well formed or not.
pub fn arb_history() -> impl Strategy<Value = Vec<CanonicalMessage>> {
    prop::collection::vec((0u8..4, prop::collection::vec(any::<u8>(), 0..4)), 0..14).prop_map(build)
}

/// A history that is already strictly valid for the Messages API: user and
/// assistant alternate, and every call is answered by the next turn, results
/// first.
pub fn arb_valid_history() -> impl Strategy<Value = Vec<CanonicalMessage>> {
    prop::collection::vec((0usize..3, any::<bool>(), any::<bool>()), 1..6).prop_map(|rounds| {
        let mut out = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        for (round, (calls, think, image_first)) in rounds.into_iter().enumerate() {
            let mut user: Vec<CanonicalContent> = pending.drain(..).map(tool_result).collect();
            if image_first {
                user.push(image());
            }
            user.push(text(format!("question {round}")));
            out.push(message(Role::User, user));
            let mut assistant = Vec::new();
            if think {
                assistant.push(thinking(true, round));
            }
            assistant.push(text(format!("answer {round}")));
            for c in 0..calls {
                let id = format!("call_{round}_{c}");
                pending.push(id.clone());
                assistant.push(tool_use(id));
            }
            out.push(message(Role::Assistant, assistant));
        }
        if !pending.is_empty() {
            out.push(message(
                Role::User,
                pending.into_iter().map(tool_result).collect(),
            ));
        }
        out
    })
}
