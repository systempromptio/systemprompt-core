//! The `opencode run --format json` wire shapes, after `OpenCode` 2.1.3's
//! JSONL stream: `{type, timestamp, sessionID, part}` per line, plus
//! `{"type":"error","error":{...}}`. Only the fields the decoder reads are
//! declared; unknown events and parts fall to `Other`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum StreamEvent {
    StepStart {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
    },
    Text {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
        part: TextPart,
    },
    ToolUse {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
        part: ToolPart,
    },
    StepFinish {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
        part: StepFinishPart,
    },
    Error {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
        error: ErrorBody,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub(super) struct TextPart {
    #[serde(default)]
    pub(super) text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolPart {
    #[serde(default, rename = "partID")]
    pub(super) part_id: Option<String>,
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) tool: String,
    pub(super) state: ToolState,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolState {
    #[serde(default)]
    pub(super) status: String,
    // JSON: the tool's own input, relayed verbatim into `ToolUse::input`.
    #[serde(default)]
    pub(super) input: serde_json::Value,
    #[serde(default)]
    pub(super) output: Option<String>,
    #[serde(default)]
    pub(super) error: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct StepFinishPart {
    #[serde(default)]
    pub(super) tokens: Tokens,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct Tokens {
    #[serde(default)]
    pub(super) input: i64,
    #[serde(default)]
    pub(super) output: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorBody {
    #[serde(default, rename = "type")]
    pub(super) kind: Option<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
}
