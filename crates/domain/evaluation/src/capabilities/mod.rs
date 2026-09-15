//! Native evaluator admission and versioned capability declarations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::experiments::{ClientKind, ExperimentSpec, VariantSpec, invalid};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_REGISTRY_VERSION: u16 = 3;
pub mod proofs;
pub use systemprompt_models::feedback::EvaluatorClient;

impl From<ClientKind> for EvaluatorClient {
    fn from(value: ClientKind) -> Self {
        match value {
            ClientKind::ClaudeCode => Self::ClaudeCode,
            ClientKind::Opencode => Self::OpenCode,
            ClientKind::Codex => Self::Codex,
            ClientKind::Hermes => Self::Hermes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    Available,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedCapabilityReason {
    AdapterNotVerified,
    InteractiveHost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluatorCapability {
    pub client: EvaluatorClient,
    pub registry_version: u16,
    pub installation_supported: bool,
    pub automated_evaluation: CapabilityAvailability,
    pub reason: Option<UnsupportedCapabilityReason>,
    pub verified_targets: Vec<VerifiedNativeTarget>,
    pub observed_readiness: Vec<NativeReadiness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NativeReadinessState {
    Unknown,
    LastVerified,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NativeReadiness {
    pub target: VerifiedNativeTarget,
    pub state: NativeReadinessState,
    pub observed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifiedNativeTarget {
    pub client: ClientKind,
    pub platform: String,
    pub architecture: String,
    pub client_version: String,
    pub adapter_version: String,
    pub image_digest: String,
    pub executable_digest: String,
    pub native_isolation_evidence_digest: String,
    pub native_metering_evidence_digest: String,
}

impl VerifiedNativeTarget {
    pub fn validate(&self) -> crate::Result<()> {
        for digest in [
            &self.image_digest,
            &self.executable_digest,
            &self.native_isolation_evidence_digest,
            &self.native_metering_evidence_digest,
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid(
                    "Native admission requires exact image, executable and native proof digests",
                ));
            }
        }
        for value in [
            &self.platform,
            &self.architecture,
            &self.client_version,
            &self.adapter_version,
        ] {
            if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
                return Err(invalid(
                    "Native admission requires bounded exact platform and versions",
                ));
            }
        }
        Ok(())
    }

    pub fn supports_platform(&self, platform: &str, architecture: &str) -> bool {
        self.platform == platform && self.architecture == architecture
    }

    pub fn matches(&self, variant: &VariantSpec, platform: &str, architecture: &str) -> bool {
        self.validate().is_ok()
            && self.client == variant.client
            && self.client_version == variant.client_version
            && self.supports_platform(platform, architecture)
            && self.image_digest == variant.worker_image_digest
    }
}

pub fn verified_native_targets() -> &'static [VerifiedNativeTarget] {
    proofs::reviewed_targets()
}

pub fn evaluator_capabilities() -> Vec<EvaluatorCapability> {
    [
        EvaluatorClient::ClaudeCode,
        EvaluatorClient::OpenCode,
        EvaluatorClient::Codex,
        EvaluatorClient::Hermes,
        EvaluatorClient::ClaudeDesktop,
    ]
    .into_iter()
    .map(|client| {
        let verified_targets: Vec<_> = verified_native_targets()
            .iter()
            .filter(|target| {
                EvaluatorClient::from(target.client) == client
                    && target.supports_platform(std::env::consts::OS, std::env::consts::ARCH)
            })
            .cloned()
            .collect();
        let available = !verified_targets.is_empty();
        EvaluatorCapability {
            client,
            registry_version: CAPABILITY_REGISTRY_VERSION,
            installation_supported: true,
            automated_evaluation: if available {
                CapabilityAvailability::Available
            } else {
                CapabilityAvailability::Unsupported
            },
            reason: if available {
                None
            } else if client == EvaluatorClient::ClaudeDesktop {
                Some(UnsupportedCapabilityReason::InteractiveHost)
            } else {
                Some(UnsupportedCapabilityReason::AdapterNotVerified)
            },
            observed_readiness: verified_targets
                .iter()
                .cloned()
                .map(|target| NativeReadiness {
                    target,
                    state: NativeReadinessState::Unknown,
                    observed_at: None,
                    diagnostic: None,
                })
                .collect(),
            verified_targets,
        }
    })
    .collect()
}

pub fn admit_variant(variant: &VariantSpec) -> crate::Result<&'static VerifiedNativeTarget> {
    verified_native_targets().iter().find(|target| target.matches(variant,
        std::env::consts::OS, std::env::consts::ARCH))
        .ok_or_else(|| invalid("Automated evaluator target is unsupported: exact native isolation and gateway metering proofs are required before reserving budget"))
}

pub fn admit_experiment(spec: &ExperimentSpec) -> crate::Result<()> {
    spec.validate()?;
    paired_variants(spec)?;
    for variant in &spec.variants {
        admit_variant(variant)?;
    }
    Ok(())
}

pub fn paired_variants(spec: &ExperimentSpec) -> crate::Result<(&VariantSpec, &VariantSpec)> {
    if spec.variants.len() != 2 {
        return Err(invalid(
            "Paired comparison requires baseline and candidate variants",
        ));
    }
    let baseline = &spec.variants[0];
    let candidate = &spec.variants[1];
    if baseline.client != candidate.client
        || baseline.client_version != candidate.client_version
        || baseline.model != candidate.model
        || baseline.provider != candidate.provider
        || baseline.configuration_digest != candidate.configuration_digest
        || baseline.worker_image_digest != candidate.worker_image_digest
        || baseline.skill_bundle_digest == candidate.skill_bundle_digest
    {
        return Err(invalid(
            "Only the candidate skill bundle may differ between paired variants",
        ));
    }
    Ok((baseline, candidate))
}

pub trait ExecutionAdmission: Send + Sync + std::fmt::Debug {
    fn admit(&self, spec: &ExperimentSpec) -> crate::Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VerifiedExecutionAdmission;

impl ExecutionAdmission for VerifiedExecutionAdmission {
    fn admit(&self, spec: &ExperimentSpec) -> crate::Result<()> {
        admit_experiment(spec)
    }
}
