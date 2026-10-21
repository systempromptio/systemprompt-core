//! Turn normalisation applied to a canonical history before any upstream
//! encoder renders it.
//!
//! Clients disagree on how a tool round trip is laid out. `OpenAI` Chat
//! clients (`OpenCode`, Cursor) send one `role: "tool"` message per parallel
//! call; Anthropic clients send every result in one user turn. Anthropic's own
//! API merges consecutive same-role turns, but strict Anthropic-compatible
//! upstreams (`DeepSeek`, `MiniMax`) and every other wire do not, and they
//! reject a `tool_use` that is not answered by the very next turn. One shared
//! pass therefore:
//!
//! - merges consecutive turns on the same side of the conversation;
//! - puts the tool results first in a merged user turn;
//! - answers an unanswered `tool_use` with a synthetic error result;
//! - drops a tool result that answers no `tool_use` of the preceding turn.
//!
//! Every other part — thinking signatures, images, cache breakpoints — is kept
//! in its original order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::content::{CanonicalContent, CanonicalMessage, Role};

pub const INTERRUPTED_TOOL_RESULT: &str = "tool call interrupted";

/// Which non-assistant roles a wire folds into one user turn: `UserSide`
/// (Anthropic, Gemini) folds system, tool and user alike; `UserAndTool`
/// (the `OpenAI` wires) keeps a mid-history system message as its own item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnMerge {
    UserSide,
    UserAndTool,
}

impl TurnMerge {
    const fn side(self, role: Role) -> Side {
        match (self, role) {
            (_, Role::Assistant) => Side::Assistant,
            (Self::UserAndTool, Role::System) => Side::System,
            _ => Side::User,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Assistant,
    User,
    System,
}

#[must_use]
pub fn normalize_turns(messages: &[CanonicalMessage], merge: TurnMerge) -> Vec<CanonicalMessage> {
    let merged = merge_runs(messages.iter().cloned(), merge);
    let repaired = repair_tool_pairing(merged, merge);
    merge_runs(
        repaired.into_iter().filter(|m| !m.content.is_empty()),
        merge,
    )
}

fn merge_runs(
    messages: impl Iterator<Item = CanonicalMessage>,
    merge: TurnMerge,
) -> Vec<CanonicalMessage> {
    let mut out: Vec<CanonicalMessage> = Vec::new();
    for msg in messages {
        let side = merge.side(msg.role);
        match out.last_mut() {
            Some(last) if merge.side(last.role) == side && side != Side::System => {
                last.role = merged_role(last.role, msg.role);
                last.content.extend(msg.content);
            },
            _ => out.push(msg),
        }
    }
    for msg in &mut out {
        if merge.side(msg.role) == Side::User {
            results_first(&mut msg.content);
        }
    }
    out
}

// Why: an OpenAI Chat `role: "tool"` message renders only its tool results,
// so a run that also holds user text must render through the user path.
fn merged_role(left: Role, right: Role) -> Role {
    if left == right { left } else { Role::User }
}

fn results_first(content: &mut Vec<CanonicalContent>) {
    let (mut results, rest): (Vec<_>, Vec<_>) = std::mem::take(content)
        .into_iter()
        .partition(|part| matches!(part, CanonicalContent::ToolResult { .. }));
    results.extend(rest);
    *content = results;
}

fn tool_use_ids(msg: &CanonicalMessage) -> Vec<String> {
    msg.content
        .iter()
        .filter_map(|part| match part {
            CanonicalContent::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn repair_tool_pairing(messages: Vec<CanonicalMessage>, merge: TurnMerge) -> Vec<CanonicalMessage> {
    let mut out: Vec<CanonicalMessage> = Vec::with_capacity(messages.len() + 1);
    let mut pending: Vec<String> = Vec::new();
    let mut server_tools = false;
    for mut msg in messages {
        match merge.side(msg.role) {
            Side::Assistant => {
                if !pending.is_empty() {
                    out.push(interrupted_turn(&pending));
                }
                pending = tool_use_ids(&msg);
                server_tools = has_server_tool_block(&msg);
            },
            Side::User => {
                answer_pending(&mut msg.content, &pending, server_tools);
                pending.clear();
            },
            Side::System => {
                if !pending.is_empty() {
                    out.push(interrupted_turn(&pending));
                    pending.clear();
                }
            },
        }
        out.push(msg);
    }
    if !pending.is_empty() {
        out.push(interrupted_turn(&pending));
    }
    out
}

// Why: an Anthropic server tool (tool search, web search) pairs its own
// blocks upstream, so a result after one is not provably orphaned and is kept.
fn has_server_tool_block(msg: &CanonicalMessage) -> bool {
    msg.content
        .iter()
        .any(|part| matches!(part, CanonicalContent::AnthropicToolBlock { .. }))
}

fn answer_pending(content: &mut Vec<CanonicalContent>, pending: &[String], keep_unpaired: bool) {
    let mut answered: HashSet<String> = HashSet::new();
    content.retain(|part| match part {
        CanonicalContent::ToolResult { tool_use_id, .. } => {
            (pending.contains(tool_use_id) || keep_unpaired) && answered.insert(tool_use_id.clone())
        },
        _ => true,
    });
    let missing: Vec<CanonicalContent> = pending
        .iter()
        .filter(|id| !answered.contains(*id))
        .map(|id| interrupted_result(id))
        .collect();
    let at = content
        .iter()
        .take_while(|part| matches!(part, CanonicalContent::ToolResult { .. }))
        .count();
    content.splice(at..at, missing);
}

fn interrupted_turn(pending: &[String]) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content: pending.iter().map(|id| interrupted_result(id)).collect(),
    }
}

fn interrupted_result(tool_use_id: &str) -> CanonicalContent {
    CanonicalContent::ToolResult {
        tool_use_id: tool_use_id.to_owned(),
        content: vec![CanonicalContent::text(INTERRUPTED_TOOL_RESULT)],
        is_error: true,
        structured_content: None,
        meta: None,
        cache_control: None,
    }
}
