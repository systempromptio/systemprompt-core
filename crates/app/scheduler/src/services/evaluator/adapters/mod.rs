//! Native adapters provide client configuration and evidence within the shared
//! supervisor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod claude_code;
pub mod codex;
pub mod hermes;
pub mod opencode;

use super::client::ClientPurpose;
use std::ffi::OsString;
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::{
    ArtifactFile, EvidenceArchive, ExecutionLimits,
};
use systemprompt_identifiers::{EvalExecutionId, ModelId, SessionId};

#[derive(Debug)]
pub struct AdapterInvocation<'a> {
    pub model: &'a ModelId,
    pub limits: &'a ExecutionLimits,
    pub purpose: ClientPurpose,
    pub prompt: &'a str,
}

pub struct AdapterContext<'a> {
    pub relay_url: &'a str,
    pub execution_token: &'a str,
    pub session_id: &'a SessionId,
    pub execution_id: &'a EvalExecutionId,
    pub target: &'a systemprompt_evaluation::capabilities::VerifiedNativeTarget,
    pub frozen: &'a systemprompt_evaluation::experiments::FrozenSettings,
}

impl std::fmt::Debug for AdapterContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterContext")
            .field("session_id", self.session_id)
            .field("execution_id", self.execution_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeCompletion {
    Completed,
    Failed,
    Incomplete,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedClientOutput {
    pub text: String,
    pub completion: NativeCompletion,
    pub reported_input_tokens: Option<u64>,
    pub reported_output_tokens: Option<u64>,
    pub tool_calls: Vec<String>,
}

impl NormalizedClientOutput {
    pub fn validate(&self) -> systemprompt_evaluation::Result<()> {
        if self.text.len() > 16 * 1024 * 1024
            || self.tool_calls.len() > 1000
            || self
                .tool_calls
                .iter()
                .any(|name| name.len() > 256 || name.chars().any(char::is_control))
        {
            return Err(EvaluationError::InvalidSpec(
                "Native output exceeds normalized evidence bounds".to_owned(),
            ));
        }
        Ok(())
    }
}

pub trait NativeAdapter: Sync + std::fmt::Debug {
    fn client(&self) -> ClientKind;
    fn adapter_version(&self) -> &'static str;
    fn executable(&self) -> &'static str;
    fn skill_directory(&self) -> &'static str;
    fn arguments(
        &self,
        input: &AdapterInvocation<'_>,
    ) -> systemprompt_evaluation::Result<Vec<OsString>>;
    fn configuration(
        &self,
        context: &AdapterContext<'_>,
    ) -> systemprompt_evaluation::Result<EvidenceArchive>;
    fn version_arguments(&self) -> Vec<OsString>;
    fn parse_version(&self, output: &[u8]) -> systemprompt_evaluation::Result<String>;
    fn normalize(&self, output: &[u8]) -> systemprompt_evaluation::Result<NormalizedClientOutput>;
}

static REGISTERED_ADAPTERS: [&dyn NativeAdapter; 4] = [
    &claude_code::ADAPTER,
    &opencode::ADAPTER,
    &codex::ADAPTER,
    &hermes::ADAPTER,
];

pub fn registered_adapters() -> &'static [&'static dyn NativeAdapter] {
    &REGISTERED_ADAPTERS
}

pub fn adapter(kind: ClientKind) -> systemprompt_evaluation::Result<&'static dyn NativeAdapter> {
    registered_adapters()
        .iter()
        .copied()
        .find(|adapter| adapter.client() == kind)
        .ok_or_else(|| {
            EvaluationError::InvalidSpec("Native client adapter is unavailable".to_owned())
        })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NormalizationEvidence {
    #[serde(flatten)]
    pub output: NormalizedClientOutput,
    pub diagnostic: Option<String>,
}

pub fn normalize_evidence(adapter: &dyn NativeAdapter, bytes: &[u8]) -> NormalizationEvidence {
    match adapter.normalize(bytes).and_then(|output| {
        output.validate()?;
        Ok(output)
    }) {
        Ok(output) => NormalizationEvidence {
            output,
            diagnostic: None,
        },
        Err(error) => NormalizationEvidence {
            output: NormalizedClientOutput {
                text: String::new(),
                completion: NativeCompletion::Incomplete,
                reported_input_tokens: None,
                reported_output_tokens: None,
                tool_calls: Vec::new(),
            },
            diagnostic: Some(error.to_string().chars().take(1024).collect()),
        },
    }
}

pub(super) fn invalid(message: &str) -> EvaluationError {
    EvaluationError::InvalidSpec(message.to_owned())
}

pub(super) fn malformed(message: &str, source: impl std::fmt::Display) -> EvaluationError {
    EvaluationError::InvalidSpec(format!("{message}: {source}"))
}

pub(super) const fn file(bytes: Vec<u8>) -> ArtifactFile {
    ArtifactFile {
        bytes,
        executable: false,
    }
}

pub(super) fn exact_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| exact_component(part))
}

fn exact_component(part: &str) -> bool {
    let leading_zero = part.len() > 1 && part.starts_with('0');
    !part.is_empty()
        && !leading_zero
        && part.bytes().all(|byte| byte.is_ascii_digit())
        && part.parse::<u32>().is_ok()
}
