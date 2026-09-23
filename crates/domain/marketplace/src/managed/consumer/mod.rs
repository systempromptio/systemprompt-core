//! Device-authenticated consumer evidence: credentials, installation plans,
//! receipts and session bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod credentials;
mod plan;
mod receipts;
mod sessions;

pub use credentials::IssuedConsumerCredential;
pub use receipts::verify_readback;
pub use sessions::ConsumerSessionBinding;

use systemprompt_models::feedback::EvaluatorClient;

pub const fn host_key(host: EvaluatorClient) -> &'static str {
    match host {
        EvaluatorClient::ClaudeCode => "claude-code",
        EvaluatorClient::OpenCode => "opencode",
        EvaluatorClient::Codex => "codex",
        EvaluatorClient::Hermes => "hermes",
        EvaluatorClient::ClaudeDesktop => "claude-desktop",
    }
}
