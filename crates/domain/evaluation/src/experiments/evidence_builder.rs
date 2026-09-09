//! Builders for frozen execution limits and evidence manifests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

#[derive(Debug, Default)]
pub struct ExecutionEvidenceBuilder {
    execution_id: Option<EvalExecutionId>,
    fencing_token: Option<i64>,
    capabilities: Option<ClientCapabilities>,
    installed_bundle_digest: Option<String>,
    candidate_bundle_digest: Option<String>,
    workspace_digest: Option<String>,
    requests: Option<Vec<AiRequestId>>,
    artifacts: Option<Vec<ArtifactEvidence>>,
    exit_code: Option<Option<i32>>,
    elapsed_milliseconds: Option<u64>,
    cleanup_confirmed: Option<bool>,
}
impl ExecutionEvidence {
    pub fn builder() -> ExecutionEvidenceBuilder {
        ExecutionEvidenceBuilder::default()
    }
}
impl ExecutionEvidenceBuilder {
    pub fn execution_id(mut self, value: EvalExecutionId) -> Self {
        self.execution_id = Some(value);
        self
    }
    pub fn fencing_token(mut self, value: i64) -> Self {
        self.fencing_token = Some(value);
        self
    }
    pub fn capabilities(mut self, value: ClientCapabilities) -> Self {
        self.capabilities = Some(value);
        self
    }
    pub fn installed_bundle_digest(mut self, value: String) -> Self {
        self.installed_bundle_digest = Some(value);
        self
    }
    pub fn candidate_bundle_digest(mut self, value: String) -> Self {
        self.candidate_bundle_digest = Some(value);
        self
    }
    pub fn workspace_digest(mut self, value: String) -> Self {
        self.workspace_digest = Some(value);
        self
    }
    pub fn requests(mut self, value: Vec<AiRequestId>) -> Self {
        self.requests = Some(value);
        self
    }
    pub fn artifacts(mut self, value: Vec<ArtifactEvidence>) -> Self {
        self.artifacts = Some(value);
        self
    }
    pub fn exit_code(mut self, value: Option<i32>) -> Self {
        self.exit_code = Some(value);
        self
    }
    pub fn elapsed_milliseconds(mut self, value: u64) -> Self {
        self.elapsed_milliseconds = Some(value);
        self
    }
    pub fn cleanup_confirmed(mut self, value: bool) -> Self {
        self.cleanup_confirmed = Some(value);
        self
    }
    pub fn build(self) -> Result<ExecutionEvidence> {
        let value = ExecutionEvidence {
            execution_id: self
                .execution_id
                .ok_or_else(|| invalid("execution_id is required"))?,
            fencing_token: self
                .fencing_token
                .ok_or_else(|| invalid("fencing_token is required"))?,
            capabilities: self
                .capabilities
                .ok_or_else(|| invalid("capabilities is required"))?,
            installed_bundle_digest: self
                .installed_bundle_digest
                .ok_or_else(|| invalid("installed_bundle_digest is required"))?,
            candidate_bundle_digest: self
                .candidate_bundle_digest
                .ok_or_else(|| invalid("candidate_bundle_digest is required"))?,
            workspace_digest: self
                .workspace_digest
                .ok_or_else(|| invalid("workspace_digest is required"))?,
            requests: self
                .requests
                .ok_or_else(|| invalid("requests is required"))?,
            artifacts: self
                .artifacts
                .ok_or_else(|| invalid("artifacts is required"))?,
            exit_code: self
                .exit_code
                .ok_or_else(|| invalid("exit_code is required"))?,
            elapsed_milliseconds: self
                .elapsed_milliseconds
                .ok_or_else(|| invalid("elapsed_milliseconds is required"))?,
            cleanup_confirmed: self
                .cleanup_confirmed
                .ok_or_else(|| invalid("cleanup_confirmed is required"))?,
        };
        value.validate()?;
        Ok(value)
    }
}
