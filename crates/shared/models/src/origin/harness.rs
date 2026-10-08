//! The mapping between the wire vocabulary and the evaluator harnesses the
//! feedback pipeline scores.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ClientKind, OriginParseError};
use crate::feedback::EvaluatorClient;

impl From<EvaluatorClient> for ClientKind {
    fn from(client: EvaluatorClient) -> Self {
        match client {
            EvaluatorClient::ClaudeCode => Self::ClaudeCode,
            EvaluatorClient::ClaudeDesktop => Self::ClaudeDesktop,
            EvaluatorClient::Codex => Self::Codex,
            EvaluatorClient::OpenCode => Self::OpenCode,
            EvaluatorClient::Hermes => Self::Hermes,
        }
    }
}

impl TryFrom<ClientKind> for EvaluatorClient {
    type Error = OriginParseError;

    fn try_from(kind: ClientKind) -> Result<Self, Self::Error> {
        match kind {
            ClientKind::ClaudeCode => Ok(Self::ClaudeCode),
            ClientKind::ClaudeDesktop => Ok(Self::ClaudeDesktop),
            ClientKind::Codex => Ok(Self::Codex),
            ClientKind::OpenCode => Ok(Self::OpenCode),
            ClientKind::Hermes => Ok(Self::Hermes),
            ClientKind::Pi | ClientKind::Other | ClientKind::Internal | ClientKind::Unknown => {
                Err(OriginParseError::NotAHarness(kind.as_str()))
            },
        }
    }
}
