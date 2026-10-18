//! Claude Code adapter: `claude -p --output-format stream-json --verbose`.
//!
//! [`ClaudeCode`] builds the invocation ([`args`], [`plugin`]), decodes each
//! stream-json line ([`stream_json`], [`blocks`]) and classifies the finished
//! process ([`outcome`]). The host runs in [`RunRequest::workspace`], as
//! Codex does, or the run's scratch workspace when the request names none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod args;
pub mod blocks;
pub mod outcome;
pub mod plugin;
pub mod stream_json;

use super::{
    Execution, ExitInfo, HeadlessHost, HostCapabilities, HostEvent, HostInvocation, PrepareContext,
};
use crate::error::AgentRunsResult;
use crate::event::RunEventPayload;
use crate::outcome::RunOutcome;
use crate::request::{HostKind, RunRequest};

pub const CLAUDE_PROGRAM: &str = "claude";

/// The Claude Code headless host.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCode;

impl HeadlessHost for ClaudeCode {
    fn kind(&self) -> HostKind {
        HostKind::ClaudeCode
    }

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        &[".claude/projects"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            hooks: true,
            resume: true,
            structured_output: true,
            mcp: true,
        }
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        let files = plugin::write_host_files(ctx)?;
        Ok(HostInvocation {
            program: CLAUDE_PROGRAM.to_owned(),
            args: args::claude_args(ctx.request, &files),
            env: args::child_env(ctx),
            cwd: working_dir(ctx),
            stdin: None,
        })
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        stream_json::decode_line(line)
    }

    fn outcome(
        &self,
        exit: ExitInfo,
        events: &[RunEventPayload],
        request: &RunRequest,
    ) -> RunOutcome {
        outcome::classify(exit, events, request)
    }
}

fn working_dir(ctx: &PrepareContext<'_>) -> std::path::PathBuf {
    if ctx.request.workspace.as_os_str().is_empty() {
        ctx.layout.workspace.clone()
    } else {
        ctx.request.workspace.clone()
    }
}

pub(super) fn execution() -> Execution {
    Execution::Supported(Box::new(ClaudeCode))
}
