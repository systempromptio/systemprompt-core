//! How a headless run answers the engine's permission requests and questions.
//!
//! The permission rules decide first, as in the window; the mode only says
//! what becomes of a request the rules leave to the user. `ask` takes the next
//! scripted reply and rejects once the script is used up, `allow` approves
//! each request once, `deny` rejects each one. Questions take the next
//! scripted answer in every mode and are dismissed without one. Under a
//! control feed the requests the rules leave to the user are held in
//! [`WaitingAsks`] until a `reply` names each.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use systemprompt_engine::run::Reply;
use systemprompt_engine::store::SessionId;
use systemprompt_identifiers::engine::PermissionId;

/// One question request's answers, one list per question; `None` dismisses
/// the request.
pub type QuestionAnswers = Option<Vec<Vec<String>>>;

/// What becomes of a permission request the rules leave to the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    Ask,
    Allow,
    Deny,
}

impl PermissionMode {
    pub const NAMES: &'static str = "ask, allow or deny";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ask" => Some(Self::Ask),
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

/// Where an answer came from: the mode, the script, a `reply` on the control
/// stream, or nothing (the script was used up, so the request was rejected or
/// dismissed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerSource {
    Mode,
    Scripted,
    Control,
    Unanswered,
}

/// The scripted answers of a run, consumed in the order the engine asks.
///
/// `permissions` are engine replies (`{"type":"once"}`, `{"type":"always"}`,
/// `{"type":"reject","feedback":"..."}`); `questions` are one entry per
/// question request, `null` to dismiss it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answers {
    #[serde(default)]
    pub permissions: VecDeque<Reply>,
    #[serde(default)]
    pub questions: VecDeque<QuestionAnswers>,
}

impl Answers {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// The scripted answers a run ended without using.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct UnusedAnswers {
    pub permissions: usize,
    pub questions: usize,
}

/// The permission requests of a controlled run that still wait for a `reply`,
/// each with the session that raised it, in the order they were raised.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WaitingAsks {
    asks: Vec<(PermissionId, SessionId)>,
}

impl WaitingAsks {
    pub fn hold(&mut self, id: &PermissionId, session: &SessionId) -> bool {
        if self.asks.iter().any(|(held, _)| held == id) {
            return false;
        }
        self.asks.push((id.clone(), session.clone()));
        true
    }

    pub fn release(&mut self, id: &PermissionId) -> Option<SessionId> {
        let at = self.asks.iter().position(|(held, _)| held == id)?;
        Some(self.asks.remove(at).1)
    }

    pub fn drain(&mut self) -> Vec<(PermissionId, SessionId)> {
        std::mem::take(&mut self.asks)
    }

    pub const fn len(&self) -> usize {
        self.asks.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.asks.is_empty()
    }
}

/// Answers every request of one run from its mode and its script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decider {
    mode: PermissionMode,
    answers: Answers,
}

impl Decider {
    pub const fn new(mode: PermissionMode, answers: Answers) -> Self {
        Self { mode, answers }
    }

    pub const fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn permission(&mut self) -> (Reply, AnswerSource) {
        match self.mode {
            PermissionMode::Allow => (Reply::Once, AnswerSource::Mode),
            PermissionMode::Deny => (Reply::Reject { feedback: None }, AnswerSource::Mode),
            PermissionMode::Ask => self.answers.permissions.pop_front().map_or(
                (Reply::Reject { feedback: None }, AnswerSource::Unanswered),
                |reply| (reply, AnswerSource::Scripted),
            ),
        }
    }

    pub fn question(&mut self) -> (QuestionAnswers, AnswerSource) {
        self.answers
            .questions
            .pop_front()
            .map_or((None, AnswerSource::Unanswered), |answers| {
                (answers, AnswerSource::Scripted)
            })
    }

    pub fn unused(&self) -> UnusedAnswers {
        UnusedAnswers {
            permissions: self.answers.permissions.len(),
            questions: self.answers.questions.len(),
        }
    }
}
