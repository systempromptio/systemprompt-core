//! The normalised trace of one run, built from its [`Transcript`] and the
//! scenario's script.
//!
//! Every model request becomes a `model_request` step. A request that took a
//! scripted turn without a fault is paired with the stored reply that holds the
//! turn's calls (or its text), which places it in its session; the reply then
//! gives the `assistant_text` or `compaction` step and, per call, the
//! `tool_call`, `permission`, `subagent` and `tool_result` steps. The
//! runner's actions are placed by the number of requests received when each
//! was taken.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::digest;
use crate::script::ScenarioScript;
use crate::transcript::{Call, CallStatus, MarkKind, Message, Reply, Transcript};

struct Pairing<'a> {
    replies: Vec<Option<(&'a str, &'a Reply)>>,
    sessions: Vec<String>,
    fresh: Vec<Vec<String>>,
}

fn pair<'a>(transcript: &'a Transcript, script: &ScenarioScript) -> Pairing<'a> {
    let all: Vec<(&str, &Reply)> = transcript
        .sessions
        .iter()
        .flat_map(|(id, log)| {
            log.messages
                .iter()
                .filter_map(move |message| match message {
                    Message::Assistant(reply) => Some((id.as_str(), reply)),
                    Message::User { .. } => None,
                })
        })
        .collect();
    let mut used = BTreeSet::new();
    let mut next_turn = 0usize;
    let mut replies = Vec::new();
    for request in &transcript.requests {
        if request.unscripted {
            replies.push(None);
            continue;
        }
        let turn = script.turns.get(next_turn);
        next_turn += 1;
        let Some(turn) = turn.filter(|turn| turn.status.is_none()) else {
            replies.push(None);
            continue;
        };
        let found = all.iter().position(|(_, reply)| {
            !used.contains(&reply.id) && {
                let ids: Vec<&str> = reply.calls.iter().map(|call| call.id.as_str()).collect();
                if turn.calls.is_empty() {
                    ids.is_empty() && matches_text(&turn.text, &reply.text)
                } else {
                    ids == turn.calls.iter().map(String::as_str).collect::<Vec<_>>()
                }
            }
        });
        replies.push(found.map(|at| {
            used.insert(all[at].1.id.clone());
            all[at]
        }));
    }
    let mut sessions = vec![transcript.root.clone(); replies.len()];
    let mut following = transcript.root.clone();
    for (at, reply) in replies.iter().enumerate().rev() {
        if let Some((session, _)) = reply {
            following = (*session).to_owned();
        }
        sessions[at].clone_from(&following);
    }
    let mut seen = BTreeSet::new();
    let fresh = transcript
        .requests
        .iter()
        .map(|request| {
            request
                .results
                .iter()
                .filter(|id| seen.insert((*id).clone()))
                .cloned()
                .collect()
        })
        .collect();
    Pairing {
        replies,
        sessions,
        fresh,
    }
}

fn matches_text(scripted: &str, stored: &str) -> bool {
    let (scripted, stored) = (scripted.trim(), stored.trim());
    scripted == stored || (!stored.is_empty() && scripted.starts_with(stored))
}

/// The whole trace: steps, the workspace object given, and the session.
pub fn trace(
    transcript: &Transcript,
    script: &ScenarioScript,
    protocol: &str,
    workspace: Value,
) -> Value {
    let pairing = pair(transcript, script);
    let builder = Builder {
        transcript,
        pairing: &pairing,
        protocol,
    };
    let steps = builder.session_steps(&transcript.root);
    json!({
        "steps": steps,
        "workspace": workspace,
        "session": crate::summary::session(transcript),
    })
}

struct Builder<'a> {
    transcript: &'a Transcript,
    pairing: &'a Pairing<'a>,
    protocol: &'a str,
}

impl Builder<'_> {
    fn session_steps(&self, session: &str) -> Vec<Value> {
        let root = session == self.transcript.root;
        let requests: Vec<usize> = (0..self.pairing.sessions.len())
            .filter(|at| self.pairing.sessions[*at] == session)
            .collect();
        let mut steps = Vec::new();
        let mut open = false;
        let mut reason = "end";
        for &at in &requests {
            if root {
                for mark in self
                    .transcript
                    .marks
                    .iter()
                    .filter(|mark| mark.requests_before == at)
                {
                    let delivery = match mark.kind {
                        MarkKind::Steer => "steer",
                        MarkKind::Prompt => "prompt",
                        MarkKind::Abort | MarkKind::Revert => continue,
                    };
                    if mark.kind == MarkKind::Prompt && open {
                        steps.push(json!({ "kind": "finish", "reason": reason }));
                        open = false;
                    }
                    steps.push(json!({ "kind": "user_message", "delivery": delivery, "message": mark.message }));
                }
            }
            open = true;
            reason = self.request_steps(at, &mut steps);
            if root
                && self
                    .transcript
                    .marks
                    .iter()
                    .any(|mark| mark.kind == MarkKind::Abort && mark.requests_before == at + 1)
            {
                steps.push(json!({ "kind": "abort" }));
                reason = "aborted";
            }
        }
        if open {
            steps.push(json!({ "kind": "finish", "reason": reason }));
        }
        if root {
            for mark in self
                .transcript
                .marks
                .iter()
                .filter(|mark| mark.kind == MarkKind::Revert)
            {
                steps.push(json!({ "kind": "revert", "message": mark.message }));
            }
        }
        steps
    }

    fn request_steps(&self, at: usize, steps: &mut Vec<Value>) -> &'static str {
        let request = &self.transcript.requests[at];
        let reply = self.pairing.replies[at].map(|(_, reply)| reply);
        let compaction = !request.has_tools && reply.is_some_and(|reply| reply.calls.is_empty());
        let mut step = Map::new();
        step.insert("kind".into(), json!("model_request"));
        step.insert("protocol".into(), json!(self.protocol));
        step.insert("tool_results".into(), json!(self.pairing.fresh[at]));
        if let Some(status) = request.status {
            step.insert("status".into(), json!(status));
        }
        if request.unscripted {
            step.insert("status".into(), json!(400));
        }
        if compaction && reply.is_some_and(|reply| reply.summary) {
            step.insert("purpose".into(), json!("compaction"));
        }
        steps.push(Value::Object(step));
        let Some(reply) = reply else {
            return "error";
        };
        if reply.summary {
            let auto = self.compaction_auto(self.pairing.sessions[at].as_str());
            steps.push(
                json!({ "kind": "compaction", "auto": auto, "digest": digest::text(&reply.text) }),
            );
        } else if !reply.text.trim().is_empty() {
            steps.push(json!({ "kind": "assistant_text", "digest": digest::text(&reply.text) }));
        }
        for call in &reply.calls {
            self.call_steps(call, steps);
        }
        finish_reason(reply)
    }

    fn compaction_auto(&self, session: &str) -> bool {
        self.transcript
            .session(session)
            .and_then(|log| {
                log.messages.iter().rev().find_map(|message| match message {
                    Message::User { compaction, .. } => *compaction,
                    Message::Assistant(_) => None,
                })
            })
            .unwrap_or(true)
    }

    fn call_steps(&self, call: &Call, steps: &mut Vec<Value>) {
        steps.push(
            json!({ "kind": "tool_call", "call": call.id, "tool": call.tool, "input": call.input }),
        );
        steps.push(json!({ "kind": "permission", "call": call.id, "tool": call.tool, "outcome": self.outcome(call) }));
        if let Some(child) = &call.child {
            steps.push(json!({ "kind": "subagent", "agent": call.agent, "steps": self.session_steps(child) }));
        }
        let digest = (call.status != CallStatus::Unfinished).then(|| digest::text(&call.output));
        let mut result = json!({
            "kind": "tool_result",
            "call": call.id,
            "tool": call.tool,
            "status": call.status.result(),
            "digest": digest,
        });
        if call.truncated {
            result["truncated"] = json!(true);
        }
        steps.push(result);
    }

    fn outcome(&self, call: &Call) -> &'static str {
        match self.transcript.replies.get(&call.id).map(String::as_str) {
            Some("once") => "approved",
            Some("always") => "approved_always",
            Some(_) => "rejected",
            None => match call.status {
                CallStatus::DeniedByRule => "denied_by_rule",
                CallStatus::DeniedByPolicy => "denied_by_verdict",
                CallStatus::Rejected => "rejected",
                _ => "allowed_by_rule",
            },
        }
    }
}

fn finish_reason(reply: &Reply) -> &'static str {
    if reply.error.as_deref() == Some("aborted") || reply.finish.as_deref() == Some("aborted") {
        return "aborted";
    }
    if reply.error.is_some() {
        return "error";
    }
    if reply
        .calls
        .iter()
        .any(|call| call.status == CallStatus::Rejected && !call.feedback)
    {
        return "stopped";
    }
    "end"
}
