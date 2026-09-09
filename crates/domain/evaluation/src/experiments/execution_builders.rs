//! Builders for frozen execution limits and evidence manifests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ArtifactEvidence, ClientCapabilities, ClientKind, ExecutionLimits};
use crate::Result;
use crate::experiments::invalid;
#[path = "evidence_builder.rs"]
mod evidence;
pub use evidence::ExecutionEvidenceBuilder;

#[derive(Debug, Default, Clone, Copy)]
pub struct ExecutionLimitsBuilder {
    max_turns: Option<u32>,
    max_output_tokens: Option<u32>,
    active_timeout_seconds: Option<u32>,
    max_artifact_bytes: Option<u64>,
}
impl ExecutionLimits {
    pub fn builder() -> ExecutionLimitsBuilder {
        ExecutionLimitsBuilder::default()
    }
}
impl ExecutionLimitsBuilder {
    pub const fn max_turns(mut self, value: u32) -> Self {
        self.max_turns = Some(value);
        self
    }
    pub const fn max_output_tokens(mut self, value: u32) -> Self {
        self.max_output_tokens = Some(value);
        self
    }
    pub const fn active_timeout_seconds(mut self, value: u32) -> Self {
        self.active_timeout_seconds = Some(value);
        self
    }
    pub const fn max_artifact_bytes(mut self, value: u64) -> Self {
        self.max_artifact_bytes = Some(value);
        self
    }
    pub fn build(self) -> Result<ExecutionLimits> {
        let value = ExecutionLimits {
            max_turns: self
                .max_turns
                .ok_or_else(|| invalid("max_turns is required"))?,
            max_output_tokens: self
                .max_output_tokens
                .ok_or_else(|| invalid("max_output_tokens is required"))?,
            active_timeout_seconds: self
                .active_timeout_seconds
                .ok_or_else(|| invalid("active_timeout_seconds is required"))?,
            max_artifact_bytes: self
                .max_artifact_bytes
                .ok_or_else(|| invalid("max_artifact_bytes is required"))?,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Default)]
pub struct ClientCapabilitiesBuilder {
    client: Option<ClientKind>,
    client_version: Option<String>,
    adapter_version: Option<String>,
    image_digest: Option<String>,
    supports_session_resume: Option<bool>,
}
impl ClientCapabilities {
    pub fn builder() -> ClientCapabilitiesBuilder {
        ClientCapabilitiesBuilder::default()
    }
}
impl ClientCapabilitiesBuilder {
    pub const fn client(mut self, value: ClientKind) -> Self {
        self.client = Some(value);
        self
    }
    pub fn client_version(mut self, value: String) -> Self {
        self.client_version = Some(value);
        self
    }
    pub fn adapter_version(mut self, value: String) -> Self {
        self.adapter_version = Some(value);
        self
    }
    pub fn image_digest(mut self, value: String) -> Self {
        self.image_digest = Some(value);
        self
    }
    pub const fn supports_session_resume(mut self, value: bool) -> Self {
        self.supports_session_resume = Some(value);
        self
    }
    pub fn build(self) -> Result<ClientCapabilities> {
        let value = ClientCapabilities {
            client: self.client.ok_or_else(|| invalid("client is required"))?,
            client_version: self
                .client_version
                .ok_or_else(|| invalid("client_version is required"))?,
            adapter_version: self
                .adapter_version
                .ok_or_else(|| invalid("adapter_version is required"))?,
            image_digest: self
                .image_digest
                .ok_or_else(|| invalid("image_digest is required"))?,
            supports_session_resume: self
                .supports_session_resume
                .ok_or_else(|| invalid("supports_session_resume is required"))?,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Debug, Default)]
pub struct ArtifactEvidenceBuilder {
    relative_path: Option<String>,
    sha256: Option<String>,
    bytes: Option<u64>,
}
impl ArtifactEvidence {
    pub fn builder() -> ArtifactEvidenceBuilder {
        ArtifactEvidenceBuilder::default()
    }
}
impl ArtifactEvidenceBuilder {
    pub fn relative_path(mut self, value: String) -> Self {
        self.relative_path = Some(value);
        self
    }
    pub fn sha256(mut self, value: String) -> Self {
        self.sha256 = Some(value);
        self
    }
    pub const fn bytes(mut self, value: u64) -> Self {
        self.bytes = Some(value);
        self
    }
    pub fn build(self) -> Result<ArtifactEvidence> {
        let value = ArtifactEvidence {
            relative_path: self
                .relative_path
                .ok_or_else(|| invalid("relative_path is required"))?,
            sha256: self.sha256.ok_or_else(|| invalid("sha256 is required"))?,
            bytes: self.bytes.ok_or_else(|| invalid("bytes is required"))?,
        };
        Ok(value)
    }
}
