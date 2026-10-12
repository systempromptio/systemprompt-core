//! The [`HeadlessHost`] seam: one adapter per coding-agent host.
//!
//! An adapter turns a [`PrepareContext`] into a [`HostInvocation`], decodes
//! each stdout line into [`HostEvent`]s and classifies the finished process
//! into a [`RunOutcome`]. [`host_for`] resolves the adapter for a
//! [`HostKind`], or says why that host cannot run headless.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod claude_code;
pub mod codex_cli;
pub mod hermes;
pub mod opencode;

use std::path::PathBuf;

use systemprompt_identifiers::HostSessionId;

use crate::credentials::RunCredentials;
use crate::error::HeadlessResult;
use crate::event::RunEventPayload;
use crate::layout::RunLayout;
use crate::outcome::RunOutcome;
use crate::request::{HostKind, RunRequest};

/// What a host can do headless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostCapabilities {
    pub hooks: bool,
    pub resume: bool,
    pub structured_output: bool,
    pub mcp: bool,
}

/// Everything an adapter needs to build its invocation.
#[derive(Debug)]
pub struct PrepareContext<'a> {
    pub request: &'a RunRequest,
    pub layout: &'a RunLayout,
    pub credentials: &'a RunCredentials,
    pub gateway_base_url: &'a str,
    pub hooks_file: Option<PathBuf>,
}

/// The process to spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    pub stdin: Option<String>,
}

/// One decoded stdout line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HostEvent {
    pub payloads: Vec<RunEventPayload>,
    pub session_id: Option<HostSessionId>,
}

/// How the host process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: i64,
}

/// One coding-agent host.
///
/// Held as `Box<dyn HeadlessHost>` by [`Execution`] so the executor picks the
/// adapter at runtime from the run row's `host` column.
pub trait HeadlessHost: Send + Sync {
    fn kind(&self) -> HostKind;

    fn capabilities(&self) -> HostCapabilities;

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        &[]
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> HeadlessResult<HostInvocation>;

    fn parse_line(&self, line: &str) -> Vec<HostEvent>;

    fn outcome(
        &self,
        exit: ExitInfo,
        events: &[RunEventPayload],
        request: &RunRequest,
    ) -> RunOutcome;
}

/// Whether a host can run, and the adapter when it can.
pub enum Execution {
    Supported(Box<dyn HeadlessHost>),
    Unsupported { reason: &'static str },
}

impl std::fmt::Debug for Execution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Supported(host) => f.debug_tuple("Supported").field(&host.kind()).finish(),
            Self::Unsupported { reason } => f
                .debug_struct("Unsupported")
                .field("reason", reason)
                .finish(),
        }
    }
}

#[must_use]
pub fn host_for(kind: HostKind) -> Execution {
    match kind {
        HostKind::ClaudeCode => claude_code::execution(),
        HostKind::CodexCli => codex_cli::execution(),
        HostKind::OpenCode => opencode::execution(),
        HostKind::Hermes => hermes::execution(),
    }
}
