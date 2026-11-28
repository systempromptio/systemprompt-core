//! The models an instance advertises, each with the context window the
//! model list and the profile report for it.

use systemprompt_identifiers::ModelId;
use systemprompt_wire::WireProtocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFixture {
    pub id: ModelId,
    pub protocol: WireProtocol,
    pub display_name: Option<String>,
    pub context_window: Option<u64>,
}

impl ModelFixture {
    pub fn messages(id: impl Into<String>) -> Self {
        Self::on(id, WireProtocol::Anthropic)
    }

    pub fn chat(id: impl Into<String>) -> Self {
        Self::on(id, WireProtocol::OpenAiChat)
    }

    pub fn on(id: impl Into<String>, protocol: WireProtocol) -> Self {
        Self {
            id: ModelId::new(id),
            protocol,
            display_name: None,
            context_window: None,
        }
    }

    pub fn named(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub const fn context_window(mut self, tokens: u64) -> Self {
        self.context_window = Some(tokens);
        self
    }
}
