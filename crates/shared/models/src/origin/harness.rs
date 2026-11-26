//! The mappings onto the wire vocabulary: from a bridge host (both packagings
//! of a product wear one client kind) and to and from the evaluator harnesses
//! the feedback pipeline scores.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ClientKind, OriginParseError};
use crate::bridge::host::{HostAppKind, HostKind, HostProduct};
use crate::feedback::EvaluatorClient;

impl From<HostKind> for ClientKind {
    fn from(host: HostKind) -> Self {
        match host.product() {
            HostProduct::Claude => match host.kind() {
                HostAppKind::DesktopApp => Self::ClaudeDesktop,
                HostAppKind::CliTool => Self::ClaudeCode,
            },
            HostProduct::Codex => Self::Codex,
            HostProduct::OpenCode => Self::OpenCode,
            HostProduct::Hermes => Self::Hermes,
            HostProduct::Harness => Self::Harness,
        }
    }
}

impl From<EvaluatorClient> for ClientKind {
    fn from(client: EvaluatorClient) -> Self {
        match client {
            EvaluatorClient::ClaudeCode => Self::ClaudeCode,
            EvaluatorClient::ClaudeDesktop => Self::ClaudeDesktop,
            EvaluatorClient::Codex => Self::Codex,
            EvaluatorClient::OpenCode => Self::OpenCode,
            EvaluatorClient::Hermes => Self::Hermes,
            EvaluatorClient::Harness => Self::Harness,
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
            ClientKind::Harness => Ok(Self::Harness),
            ClientKind::Pi | ClientKind::Other | ClientKind::Internal | ClientKind::Unknown => {
                Err(OriginParseError::NotAHarness(kind.as_str()))
            },
        }
    }
}
