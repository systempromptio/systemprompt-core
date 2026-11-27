//! The unified events both protocol decoders produce.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Token accounting for one call. `input` counts every prompt token, cached
/// ones included; `reasoning` is part of `output` where the protocol reports
/// it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub const fn total(&self) -> u64 {
        self.input.saturating_add(self.output)
    }
}

/// Why the model stopped answering, normalised across both protocols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Other(String),
}

/// One step of a streamed model answer.
///
/// `ReasoningDelta` with a `signature` closes a reasoning block: the signature
/// must be stored with the block's text to send it back. A `ToolCallEnd` with
/// `malformed` set carries the raw argument text that could not be read as
/// JSON; its `input` is then an empty object.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    TextDelta(String),
    ReasoningDelta {
        text: String,
        signature: Option<String>,
    },
    RedactedReasoning {
        data: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallDelta {
        id: String,
        fragment: String,
    },
    ToolCallEnd {
        id: String,
        name: String,
        // JSON: tool arguments are whatever object the model wrote for the tool's input schema.
        input: Value,
        malformed: Option<String>,
    },
    Usage(Usage),
    Finish {
        reason: FinishReason,
    },
}
