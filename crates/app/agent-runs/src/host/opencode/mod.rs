//! `OpenCode` V2 adapter: `opencode run --standalone --format json --auto`.
//!
//! [`prepare`](HeadlessHost::prepare) writes the run home's
//! `~/.config/opencode/opencode.json` through the shared
//! [`OpenCodeConfig`](systemprompt_models::bridge::opencode::OpenCodeConfig)
//! builder and the persona as the global-tier `AGENTS.md`, then builds the
//! command. Each stdout JSONL line is decoded by [`events`] and the finished
//! process classified by [`outcome`]. Only `@opencode/cli` >= 2.0.30 reads a
//! managed tier and enforces `policies`, so an older client is refused before
//! spawn through [`HeadlessHost::version_requirement`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod args;
pub mod config;
pub mod events;
pub mod outcome;
mod wire;

use std::path::Path;
use std::sync::Mutex;

use systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;

use super::{
    Execution, ExitInfo, HarnessTargets, HeadlessHost, HostCapabilities, HostEvent, HostInvocation,
    PrepareContext, VersionRequirement,
};
use crate::error::{AgentRunsError, AgentRunsResult};
use crate::event::RunEventPayload;
use crate::outcome::RunOutcome;
use crate::request::{HostKind, RunRequest};

pub const PAT_ENV: &str = "SYSTEMPROMPT_PAT";

pub const PROGRAM: &str = HostKind::OpenCode.as_str();

pub(super) fn execution() -> Execution {
    Execution::Supported(Box::new(OpenCodeCli::default()))
}

/// The `OpenCode` V2 CLI host. Holds the per-run decode state, so one
/// instance serves exactly one run.
#[derive(Debug, Default)]
pub struct OpenCodeCli {
    state: Mutex<events::DecodeState>,
}

impl HeadlessHost for OpenCodeCli {
    fn kind(&self) -> HostKind {
        HostKind::OpenCodeCli
    }

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        &[".local/share/opencode"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            hooks: false,
            resume: true,
            structured_output: true,
            mcp: true,
        }
    }

    fn harness_targets(&self) -> HarnessTargets {
        HarnessTargets::Home(".config/opencode")
    }

    fn version_requirement(&self) -> Option<VersionRequirement> {
        Some(VersionRequirement {
            args: &["--version"],
            minimum: MIN_CLIENT_VERSION,
        })
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        let paths = args::OpenCodePaths::new(ctx.layout);
        let document = config::build(ctx.request, ctx.credentials, ctx.gateway_base_url);
        write_file(&paths.config_json, &document.to_json_pretty()?)?;
        if let Some(agents_md) = args::agents_md(ctx.request) {
            let harness = match std::fs::read_to_string(&paths.agents_md) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(source) => return Err(layout_error(&paths.agents_md, source)),
            };
            let joined = if harness.trim().is_empty() {
                agents_md
            } else {
                format!("{}\n\n{agents_md}", harness.trim_end())
            };
            write_file(&paths.agents_md, &joined)?;
        }
        if let Ok(mut state) = self.state.lock() {
            state.model = ctx.request.model.clone().unwrap_or_default();
            state.cwd = Some(ctx.request.workspace.display().to_string());
        }
        Ok(args::invocation(ctx.request, ctx.credentials, &paths))
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        self.state.lock().map_or_else(
            |_| Vec::new(),
            |mut state| events::decode_line(&mut state, line),
        )
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

fn write_file(path: &Path, contents: &str) -> AgentRunsResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| layout_error(parent, source))?;
    }
    std::fs::write(path, contents).map_err(|source| layout_error(path, source))
}

fn layout_error(path: &Path, source: std::io::Error) -> AgentRunsError {
    AgentRunsError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
