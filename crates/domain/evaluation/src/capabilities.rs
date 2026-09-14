//! Versioned evaluator client capability declarations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const CAPABILITY_REGISTRY_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvaluatorClient {
    ClaudeCode,
    OpenCode,
    Codex,
    Hermes,
    ClaudeDesktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedCapabilityReason {
    AdapterNotVerified,
    InteractiveHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorCapability {
    pub client: EvaluatorClient,
    pub registry_version: u16,
    pub installation_supported: bool,
    pub automated_evaluation: CapabilityAvailability,
    pub reason: UnsupportedCapabilityReason,
}

pub fn evaluator_capabilities() -> Vec<EvaluatorCapability> {
    [
        (
            EvaluatorClient::ClaudeCode,
            true,
            UnsupportedCapabilityReason::AdapterNotVerified,
        ),
        (
            EvaluatorClient::OpenCode,
            true,
            UnsupportedCapabilityReason::AdapterNotVerified,
        ),
        (
            EvaluatorClient::Codex,
            true,
            UnsupportedCapabilityReason::AdapterNotVerified,
        ),
        (
            EvaluatorClient::Hermes,
            true,
            UnsupportedCapabilityReason::AdapterNotVerified,
        ),
        (
            EvaluatorClient::ClaudeDesktop,
            true,
            UnsupportedCapabilityReason::InteractiveHost,
        ),
    ]
    .into_iter()
    .map(
        |(client, installation_supported, reason)| EvaluatorCapability {
            client,
            registry_version: CAPABILITY_REGISTRY_VERSION,
            installation_supported,
            automated_evaluation: CapabilityAvailability::Unsupported,
            reason,
        },
    )
    .collect()
}
