//! Codex CLI adapter: `codex exec --json`.
//!
//! [`prepare`](HeadlessHost::prepare) writes the run's `CODEX_HOME/config.toml`
//! (gateway provider, sandbox, MCP servers), the persona as the workspace
//! `AGENTS.md` and the output schema file, then builds the `codex exec`
//! command. Each stdout JSONL line is decoded by [`events`] and the finished
//! process classified by [`outcome`]. Codex has no hook surface, so a run is
//! governed only at the gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod args;
pub mod config_toml;
pub mod events;
pub mod outcome;
mod wire;

use std::path::Path;
use std::sync::Mutex;

use super::{
    Execution, ExitInfo, HeadlessHost, HostCapabilities, HostEvent, HostInvocation, PrepareContext,
};
use crate::error::{HeadlessError, HeadlessResult};
use crate::event::RunEventPayload;
use crate::outcome::RunOutcome;
use crate::request::{HostKind, RunRequest};

pub const PAT_ENV: &str = "SYSTEMPROMPT_PAT";

pub const PROGRAM: &str = "codex";

pub(super) fn execution() -> Execution {
    Execution::Supported(Box::new(CodexCli::default()))
}

/// The Codex CLI host. Holds the per-run decode state, so one instance
/// serves exactly one run.
#[derive(Debug, Default)]
pub struct CodexCli {
    state: Mutex<events::DecodeState>,
}

impl HeadlessHost for CodexCli {
    fn kind(&self) -> HostKind {
        HostKind::CodexCli
    }

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        &[".codex/sessions"]
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            hooks: false,
            resume: true,
            structured_output: true,
            mcp: true,
        }
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> HeadlessResult<HostInvocation> {
        let paths = args::CodexPaths::new(ctx.layout);
        write_file(
            &paths.config_toml,
            &config_toml::render(ctx.request, ctx.credentials, ctx.gateway_base_url),
        )?;
        if let Some(agents_md) = args::agents_md(ctx.request) {
            write_file(&ctx.request.workspace.join("AGENTS.md"), &agents_md)?;
        }
        if let Some(schema) = &ctx.request.output_schema {
            write_file(&paths.output_schema, &serde_json::to_string_pretty(schema)?)?;
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

fn write_file(path: &Path, contents: &str) -> HeadlessResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| layout_error(parent, source))?;
    }
    std::fs::write(path, contents).map_err(|source| layout_error(path, source))
}

fn layout_error(path: &Path, source: std::io::Error) -> HeadlessError {
    HeadlessError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
