//! `comms-drain` — the hook side of team-comms delivery.
//!
//! Claude Code hands a hook its event payload on stdin and reads the hook's
//! answer from stdout. This command reads the session id from that payload,
//! drains the inbox file the proxy wrote for that session, and answers with
//! `additionalContext` — which is what puts the message in front of the agent.
//!
//! It is a subcommand rather than a shipped shell script for two reasons: a
//! script would have to be POSIX-portable across macOS and Linux and Windows,
//! and it would need to locate the inbox directory the same way the bridge
//! does. Both are solved by reusing the binary that wrote the file.
//!
//! Draining is destructive and deliberately so. A message is surfaced once;
//! re-emitting it on every prompt is how a notification becomes noise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read;
use std::process::ExitCode;

use serde::Deserialize;

use crate::stdio;

#[derive(Debug, Deserialize)]
struct HookPayload {
    #[serde(default)]
    session_id: Option<crate::ids::HookSessionId>,
}

#[derive(Debug, Deserialize)]
struct InboxLine {
    from: String,
    preview: String,
}

pub fn cmd_comms_drain() -> ExitCode {
    let mut stdin = String::new();
    if std::io::stdin().read_to_string(&mut stdin).is_err() {
        return ExitCode::SUCCESS;
    }

    let Ok(payload) = serde_json::from_str::<HookPayload>(&stdin) else {
        return ExitCode::SUCCESS;
    };
    let Some(session_id) = payload.session_id.filter(|s| !s.as_str().is_empty()) else {
        return ExitCode::SUCCESS;
    };

    let messages = drain(&session_id);
    if messages.is_empty() {
        return ExitCode::SUCCESS;
    }

    let context = messages
        .iter()
        .map(|m| format!("[comms] @{} — {}", m.from, m.preview))
        .collect::<Vec<_>>()
        .join("\n");

    let answer = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": context,
        }
    });
    stdio::print_str(&format!("{answer}\n"));
    ExitCode::SUCCESS
}

// Why: rename-then-read rather than read-then-remove: the proxy appends to this
// file concurrently, and a message landing between a read and a remove would be
// unlinked unread. Renaming first makes the swap atomic — the appender's next
// write recreates the inbox and is delivered by the following drain, and the
// pid in the taken name keeps two concurrent drains off each other's file.
fn drain(session_id: &crate::ids::HookSessionId) -> Vec<InboxLine> {
    let safe: String = session_id
        .as_str()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() {
        return Vec::new();
    }
    let Some(path) = crate::proxy::comms::inbox_dir().map(|d| d.join(format!("{safe}.jsonl")))
    else {
        return Vec::new();
    };
    let taken = path.with_extension(format!("jsonl.{}.draining", std::process::id()));
    if std::fs::rename(&path, &taken).is_err() {
        return Vec::new();
    }
    let Ok(body) = std::fs::read_to_string(&taken) else {
        return Vec::new();
    };
    if let Err(e) = std::fs::remove_file(&taken) {
        tracing::warn!(error = %e, "could not clear the drained comms inbox");
    }

    body.lines()
        .filter_map(|line| serde_json::from_str::<InboxLine>(line).ok())
        .collect()
}
