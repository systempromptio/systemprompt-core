//! The models an instance advertises, each with the context window the
//! model list and the profile report for it, and the client family it is
//! served to.

use systemprompt_identifiers::ModelId;
use systemprompt_models::providers::ClientFamily;
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

    /// The client family core's gateway lists a model of this protocol to:
    /// what the manifest's `host_model_protocols` and a profile provider's
    /// `clients` name.
    pub const fn family(&self) -> ClientFamily {
        match self.protocol {
            WireProtocol::Anthropic => ClientFamily::Claude,
            WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => ClientFamily::OpenAi,
            WireProtocol::Gemini => ClientFamily::Gemini,
        }
    }
}
