//! Reviewed native acceptance provenance and immutable image identities.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::VerifiedNativeTarget;
use crate::Result;
use crate::experiments::invalid;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::LazyLock;

const VERSION: u16 = 1;
const MAX_DOCUMENT: usize = 1_048_576;
const ISOLATION: &[&str] = &[
    "pinned_executable",
    "configuration_credentials",
    "forbidden_tools",
    "network",
    "cancellation_cleanup",
    "attempt_bound",
    "output_bound",
    "failed_evidence",
    "matched_environments",
    "judge",
    "suggestion",
];
const METERING: &[&str] = &[
    "authenticated_execution_session",
    "atomic_reservation",
    "native_usage_parity",
    "failed_spend",
    "unknown_pricing",
    "attempt_output_cost_bounds",
    "stale_lease",
    "revocation",
    "idempotent_settlement",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofBinding {
    pub client: crate::experiments::ClientKind,
    pub platform: String,
    pub architecture: String,
    pub client_version: String,
    pub adapter_version: String,
    pub image_config_digest: String,
    pub executable_digest: String,
    pub core_source_digest: String,
    pub astound_source_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeProofReport {
    pub schema_version: u16,
    pub binding: ProofBinding,
    pub contracts: BTreeMap<String, bool>,
    pub artifacts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeProofManifest {
    pub schema_version: u16,
    pub target: VerifiedNativeTarget,
    pub binding: ProofBinding,
    pub repository_manifest_digest: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedNativeProof {
    pub manifest: &'static str,
    pub manifest_sha256: &'static str,
    pub isolation: &'static str,
    pub metering: &'static str,
}

const REVIEWED: &[EmbeddedNativeProof] = &[];
static MANIFESTS: LazyLock<Vec<NativeProofManifest>> = LazyLock::new(|| {
    REVIEWED
        .iter()
        .filter_map(|proof| match proof.validate() {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                tracing::error!(
                    error = %error,
                    manifest_sha256 = proof.manifest_sha256,
                    "Embedded native acceptance proof rejected; it is not a verified target"
                );
                None
            },
        })
        .collect()
});
static TARGETS: LazyLock<Vec<VerifiedNativeTarget>> = LazyLock::new(|| {
    MANIFESTS
        .iter()
        .map(|manifest| manifest.target.clone())
        .collect()
});

pub(super) fn reviewed_targets() -> &'static [VerifiedNativeTarget] {
    &TARGETS
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl EmbeddedNativeProof {
    pub fn validate(&self) -> Result<NativeProofManifest> {
        for document in [self.manifest, self.isolation, self.metering] {
            if document.is_empty() || document.len() > MAX_DOCUMENT {
                return Err(invalid("Native proof document exceeds bounds"));
            }
        }
        if !digest(self.manifest_sha256)
            || hex::encode(Sha256::digest(self.manifest.as_bytes())) != self.manifest_sha256
        {
            return Err(invalid("Embedded native manifest digest mismatch"));
        }
        let manifest: NativeProofManifest = serde_json::from_str(self.manifest)?;
        manifest.target.validate()?;
        if !binding_matches_target(&manifest) {
            return Err(invalid(
                "Native proof identity or image provenance mismatch",
            ));
        }
        let target = &manifest.target;
        for (document, expected, contracts) in [
            (
                self.isolation,
                &target.native_isolation_evidence_digest,
                ISOLATION,
            ),
            (
                self.metering,
                &target.native_metering_evidence_digest,
                METERING,
            ),
        ] {
            if hex::encode(Sha256::digest(document.as_bytes())) != *expected {
                return Err(invalid("Native acceptance report digest mismatch"));
            }
            let report: NativeProofReport = serde_json::from_str(document)?;
            if !report_is_complete(&report, &manifest.binding, contracts) {
                return Err(invalid(
                    "Native acceptance report is incomplete or mismatched",
                ));
            }
        }
        Ok(manifest)
    }
}

fn binding_matches_target(manifest: &NativeProofManifest) -> bool {
    let binding = &manifest.binding;
    let target = &manifest.target;
    manifest.schema_version == VERSION
        && binding.client == target.client
        && binding.platform == target.platform
        && binding.architecture == target.architecture
        && binding.client_version == target.client_version
        && binding.adapter_version == target.adapter_version
        && binding.executable_digest == target.executable_digest
        && digest(&binding.image_config_digest)
        && digest(&binding.core_source_digest)
        && digest(&binding.astound_source_digest)
        && manifest
            .repository_manifest_digest
            .as_ref()
            .is_none_or(|value| digest(value))
        && target.image_digest
            == *manifest
                .repository_manifest_digest
                .as_ref()
                .unwrap_or(&binding.image_config_digest)
}

fn report_is_complete(
    report: &NativeProofReport,
    binding: &ProofBinding,
    contracts: &[&str],
) -> bool {
    report.schema_version == VERSION
        && report.binding == *binding
        && contracts
            .iter()
            .all(|contract| report.contracts.get(*contract) == Some(&true))
        && report.contracts.values().all(|passed| *passed)
        && report.contracts.len() <= 64
        && !report.artifacts.is_empty()
        && report.artifacts.len() <= 256
        && report.artifacts.iter().all(|(name, hash)| {
            !name.is_empty()
                && name.len() <= 256
                && !name.starts_with('/')
                && name.split('/').all(|part| part != ".." && !part.is_empty())
                && !name.chars().any(char::is_control)
                && digest(hash)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImmutableImage<'a> {
    LocalConfig(&'a str),
    RepositoryManifest(&'a str),
}

impl<'a> ImmutableImage<'a> {
    pub fn parse(value: &'a str) -> Result<Self> {
        if let Some(hash) = value.strip_prefix("sha256:") {
            if digest(hash) {
                return Ok(Self::LocalConfig(hash));
            }
        } else if let Some((repository, hash)) = value.rsplit_once("@sha256:")
            && !repository.is_empty()
            && repository.len() <= 255
            && !repository.starts_with('-')
            && repository
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-/:".contains(&byte))
            && digest(hash)
        {
            return Ok(Self::RepositoryManifest(hash));
        }
        Err(invalid(
            "Expected exact local image config ID or repository manifest digest",
        ))
    }
    pub const fn digest(self) -> &'a str {
        match self {
            Self::LocalConfig(value) | Self::RepositoryManifest(value) => value,
        }
    }
}

pub fn image_config_for_target(target: &VerifiedNativeTarget, image: &str) -> Result<&'static str> {
    let image = ImmutableImage::parse(image)?;
    MANIFESTS
        .iter()
        .find(|manifest| {
            manifest.target == *target
                && match image {
                    ImmutableImage::LocalConfig(hash) => {
                        manifest.repository_manifest_digest.is_none()
                            && manifest.binding.image_config_digest == hash
                    },
                    ImmutableImage::RepositoryManifest(hash) => {
                        manifest.repository_manifest_digest.as_deref() == Some(hash)
                    },
                }
        })
        .map(|manifest| manifest.binding.image_config_digest.as_str())
        .ok_or_else(|| invalid("Image reference lacks reviewed manifest-to-config provenance"))
}
