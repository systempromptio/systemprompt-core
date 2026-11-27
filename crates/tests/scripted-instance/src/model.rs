//! The models an instance advertises.

use systemprompt_identifiers::ModelId;
use systemprompt_wire::WireProtocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFixture {
    pub id: ModelId,
    pub protocol: WireProtocol,
    pub display_name: Option<String>,
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
        }
    }

    pub fn named(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }
}
