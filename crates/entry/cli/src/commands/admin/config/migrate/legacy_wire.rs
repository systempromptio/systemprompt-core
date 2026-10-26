//! The pre-0.73 spellings of a provider entry's `wire`.
//!
//! 0.72 accepted `openai`, `openai_chat` and `openai_responses` beside the
//! canonical tags. [`WireProtocol`] reads the canonical tag only, so the old
//! spellings are parsed here, inside `config migrate`, and nowhere else.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Deserializer};
use systemprompt_wire::WireProtocol;

#[derive(Debug, Clone, Copy, Deserialize)]
enum LegacyWire {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai-chat")]
    OpenAiChat,
    #[serde(rename = "openai_chat")]
    OpenAiChatUnderscore,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openai-responses")]
    OpenAiResponses,
    #[serde(rename = "openai_responses")]
    OpenAiResponsesUnderscore,
    #[serde(rename = "gemini")]
    Gemini,
}

impl From<LegacyWire> for WireProtocol {
    fn from(wire: LegacyWire) -> Self {
        match wire {
            LegacyWire::Anthropic => Self::Anthropic,
            LegacyWire::OpenAiChat | LegacyWire::OpenAiChatUnderscore | LegacyWire::OpenAi => {
                Self::OpenAiChat
            },
            LegacyWire::OpenAiResponses | LegacyWire::OpenAiResponsesUnderscore => {
                Self::OpenAiResponses
            },
            LegacyWire::Gemini => Self::Gemini,
        }
    }
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<WireProtocol, D::Error> {
    LegacyWire::deserialize(deserializer).map(WireProtocol::from)
}
