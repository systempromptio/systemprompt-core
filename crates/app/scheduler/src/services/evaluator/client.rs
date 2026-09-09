//! Pinned native client invocations; suite data never supplies shell commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_identifiers::ModelId;

#[derive(Debug, Clone)]
pub struct NativeClient {
    kind: ClientKind,
    model: ModelId,
    limits: ExecutionLimits,
}

impl NativeClient {
    pub fn builder(kind: ClientKind, model: ModelId) -> NativeClientBuilder {
        NativeClientBuilder {
            client: Self {
                kind,
                model,
                limits: ExecutionLimits::default(),
            },
        }
    }

    pub fn arguments(&self, prompt: &str) -> Vec<OsString> {
        match self.kind {
            ClientKind::ClaudeCode => [
                "claude",
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--model",
                self.model.as_str(),
                "--max-turns",
                &self.limits.max_turns.to_string(),
                "--tools",
                "Read,Write,Edit,Glob,Grep,Skill",
                "--allowedTools",
                "Read,Write,Edit,Glob,Grep,Skill",
                "--disallowedTools",
                "Bash,Agent,Task,WebSearch,WebFetch",
                "--",
                prompt,
            ]
            .iter()
            .map(OsString::from)
            .collect(),
            ClientKind::Opencode => [
                "opencode",
                "run",
                "--format",
                "json",
                "--model",
                &format!("systemprompt/{}", self.model),
                "--",
                prompt,
            ]
            .iter()
            .map(OsString::from)
            .collect(),
        }
    }

    pub const fn limits(&self) -> &ExecutionLimits {
        &self.limits
    }
}

#[derive(Debug)]
pub struct NativeClientBuilder {
    client: NativeClient,
}

impl NativeClientBuilder {
    pub const fn limits(mut self, limits: ExecutionLimits) -> Self {
        self.client.limits = limits;
        self
    }
    pub fn build(self) -> systemprompt_evaluation::Result<NativeClient> {
        self.client.limits.validate()?;
        Ok(self.client)
    }
}
