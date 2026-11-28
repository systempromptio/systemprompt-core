//! The shell tool: runs a command in the project through the platform's shell,
//! streams its output, enforces a timeout, stops the whole process tree at the
//! end, and can leave long-running commands in the background.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod authorize;
mod capture;
mod decode;
mod jobs;
mod platform;
mod process;
mod registry;
mod run;
mod select;
mod tokenize;

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

pub use jobs::{JobInfo, JobListener, JobReport, JobState};
pub use registry::BackgroundJobs;
pub use select::{ShellKind, ShellSpec};
pub use tokenize::{Dialect, Segment, split_commands};

use crate::tools::config::ShellConfig;
use crate::tools::contract::{Tool, ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::path::{resolve, slash};
use crate::tools::repair::parse_input;
use crate::tools::text::title_line;
use crate::tools::truncate::OutputLimits;

/// Runs shell commands.
#[derive(Debug, Clone)]
pub struct ShellTool {
    spec: ShellSpec,
    config: ShellConfig,
    output: OutputLimits,
    jobs: BackgroundJobs,
}

#[derive(Debug, Deserialize)]
struct Input {
    command: String,
    timeout: Option<u64>,
    workdir: Option<String>,
    description: Option<String>,
    #[serde(default)]
    background: bool,
}

#[derive(Debug)]
struct Call<'a> {
    command: &'a str,
    cwd: &'a Path,
    description: Option<&'a str>,
    title: String,
    requested: Option<Duration>,
}

impl ShellTool {
    pub fn new(config: ShellConfig, output: OutputLimits, jobs: BackgroundJobs) -> Self {
        let spec = ShellSpec::detect(config.path.as_deref());
        tracing::info!(shell = %spec.path.display(), kind = ?spec.kind, "shell tool ready");
        Self {
            spec,
            config,
            output,
            jobs,
        }
    }

    pub const fn with_spec(
        spec: ShellSpec,
        config: ShellConfig,
        output: OutputLimits,
        jobs: BackgroundJobs,
    ) -> Self {
        Self {
            spec,
            config,
            output,
            jobs,
        }
    }

    pub const fn spec(&self) -> &ShellSpec {
        &self.spec
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        crate::tools::names::SHELL
    }

    fn description(&self) -> String {
        format!(
            "Run a command in {shell} and get back its combined output (stdout and stderr) and exit code. Commands \
             start in the working directory unless `workdir` is set; prefer `workdir` to `cd` chains. Give a short \
             `description` of what the command does. The timeout defaults to {default} seconds and can be raised \
             with `timeout` (seconds, at most {max}); when it expires the command and every process it started are \
             stopped. For servers, watchers and anything else that keeps running, set `background` to true: the \
             call returns at once with a job id and a log file, and you are told when the job exits. Very long \
             output is cut to its end, with the full text saved to a file.\n\n\
             Use the dedicated tools instead of shell equivalents: read rather than cat, type or Get-Content; glob \
             rather than find, dir or ls -R; grep rather than grep, findstr or Select-String; edit, write or patch \
             rather than sed, echo redirection or Set-Content. Commands cannot receive input, so avoid interactive \
             programs and pass non-interactive flags. Quote paths that contain spaces.\n\n{notes}",
            shell = self.spec.kind.label(),
            default = self.config.default_timeout.as_secs(),
            max = self.config.max_timeout.as_secs(),
            notes = self.spec.kind.notes(),
        )
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "The command line to run."},
                "description": {"type": "string", "description": "What the command does, in five to ten words."},
                "workdir": {"type": "string", "description": "Directory to run in. Defaults to the working directory."},
                "timeout": {"type": "integer", "minimum": 1, "description": "Timeout in seconds."},
                "background": {"type": "boolean", "description": "Run without waiting and report when it exits."}
            },
            "required": ["command"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let command = input.command.trim();
        if command.is_empty() {
            return Err(ToolError::invalid("command is empty"));
        }
        let cwd = input
            .workdir
            .as_deref()
            .map_or_else(|| ctx.cwd.clone(), |dir| resolve(&ctx.cwd, dir));
        if !tokio::fs::metadata(&cwd)
            .await
            .is_ok_and(|meta| meta.is_dir())
        {
            return Err(ToolError::invalid(format!(
                "workdir {} is not a directory",
                slash(&cwd)
            )));
        }
        let description = input
            .description
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        self.authorize(ctx, command, &cwd, description).await?;
        ctx.ensure_live()?;
        let call = Call {
            command,
            cwd: &cwd,
            description,
            title: description.map_or_else(|| title_line(command, 80), str::to_owned),
            requested: input.timeout.map(Duration::from_secs),
        };
        if input.background {
            return self.run_background(ctx, &call);
        }
        self.run_foreground(ctx, &call).await
    }
}
