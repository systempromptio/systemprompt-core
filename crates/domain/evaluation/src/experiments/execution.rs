//! Frozen native-client inputs and evidence submitted by evaluator services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ClientKind, content_digest, invalid};
#[path = "execution_builders.rs"]
mod builders;
use crate::Result;
pub use builders::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemprompt_identifiers::{AiRequestId, EvalExecutionId};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLimits {
    pub max_turns: u32,
    pub max_output_tokens: u32,
    pub active_timeout_seconds: u32,
    pub max_artifact_bytes: u64,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_turns: 12,
            max_output_tokens: 4096,
            active_timeout_seconds: 1800,
            max_artifact_bytes: 16 * 1024 * 1024,
        }
    }
}

impl ExecutionLimits {
    pub fn validate(&self) -> Result<()> {
        if !(1..=100).contains(&self.max_turns)
            || !(256..=32768).contains(&self.max_output_tokens)
            || !(1..=1800).contains(&self.active_timeout_seconds)
            || !(1..=16 * 1024 * 1024).contains(&self.max_artifact_bytes)
        {
            return Err(invalid("Execution limits exceed the supported envelope"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenWorkspace {
    pub files: BTreeMap<String, String>,
}

impl FrozenWorkspace {
    pub fn validate(&self) -> Result<()> {
        if self.files.len() > 256
            || self.files.values().map(String::len).sum::<usize>() > 8 * 1024 * 1024
        {
            return Err(invalid("Workspace exceeds 256 files or 8 MiB"));
        }
        for path in self.files.keys() {
            if path.starts_with('/')
                || path.contains(['\\', ':'])
                || path.chars().any(char::is_control)
                || path.split('/').any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(invalid("Workspace paths must be portable relative paths"));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String> {
        self.validate()?;
        content_digest(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientCapabilities {
    pub client: ClientKind,
    pub client_version: String,
    pub adapter_version: String,
    pub image_digest: String,
    pub supports_session_resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidence {
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidence {
    pub execution_id: EvalExecutionId,
    pub fencing_token: i64,
    pub capabilities: ClientCapabilities,
    pub installed_bundle_digest: String,
    pub candidate_bundle_digest: String,
    pub workspace_digest: String,
    pub requests: Vec<AiRequestId>,
    pub artifacts: Vec<ArtifactEvidence>,
    pub exit_code: Option<i32>,
    pub elapsed_milliseconds: u64,
    pub cleanup_confirmed: bool,
}

impl ClientCapabilities {
    pub fn validate(&self) -> Result<()> {
        validate_digest(&self.image_digest)?;
        for version in [&self.client_version, &self.adapter_version] {
            if version.trim().is_empty()
                || version.len() > 128
                || version.chars().any(char::is_control)
            {
                return Err(invalid(
                    "Client and adapter versions require 1–128 printable bytes",
                ));
            }
        }
        Ok(())
    }
}

impl ExecutionEvidence {
    pub fn validate(&self) -> Result<()> {
        self.capabilities.validate()?;
        if self.fencing_token < 1 || self.artifacts.len() > 256 || self.requests.len() > 1000 {
            return Err(invalid("Invalid evidence lease or manifest size"));
        }
        for digest in [
            &self.installed_bundle_digest,
            &self.candidate_bundle_digest,
            &self.workspace_digest,
            &self.capabilities.image_digest,
        ] {
            validate_digest(digest)?;
        }
        for (index, request) in self.requests.iter().enumerate() {
            if self.requests[..index].contains(request) {
                return Err(invalid("Duplicate request evidence"));
            }
        }
        for (index, artifact) in self.artifacts.iter().enumerate() {
            validate_digest(&artifact.sha256)?;
            let workspace = FrozenWorkspace {
                files: BTreeMap::from([(artifact.relative_path.clone(), String::new())]),
            };
            workspace.validate()?;
            if self.artifacts[..index]
                .iter()
                .any(|other| other.relative_path == artifact.relative_path)
            {
                return Err(invalid("Duplicate artifact evidence"));
            }
        }
        Ok(())
    }
}

fn validate_digest(digest: &str) -> Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("Expected a lowercase SHA-256 digest"));
    }
    Ok(())
}
