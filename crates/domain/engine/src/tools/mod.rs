//! The Tool contract and the built-in tools.
//!
//! A [`Tool`] is called by the engine with JSON input and a [`ToolContext`]
//! that carries where it runs and the hooks it calls back into: a
//! [`PermissionGate`] asked before anything with side effects, an optional
//! [`FileObserver`] told about every write, and an optional [`Progress`] sink
//! for live output. The engine implements those traits.
//!
//! [`builtin`] builds the standard set: `shell`, `read`, `write`, `edit`,
//! `patch`, `glob`, `grep`, `fetch` and, when a [`TodoSink`] is configured,
//! `todo`. It detects the shell once, so the set is built at startup and
//! reused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builtin;
mod config;
mod contract;
mod diff;
mod error;
mod files;
mod matching;
mod path;
mod platform;
mod repair;
mod shell;
mod text;
mod truncate;

use std::sync::Arc;

pub use builtin::{
    EditTool, FetchTool, GlobTool, GrepTool, PatchTool, ReadTool, TodoItem, TodoPriority, TodoSink,
    TodoStatus, TodoTool, WriteTool,
};
pub use config::{FetchConfig, ReadLimits, SearchLimits, ShellConfig, ToolsConfig};
pub use contract::{
    Attachment, FileObserver, PermissionGate, PermissionRequest, Progress, Tool, ToolContext,
    ToolOutput,
};
pub use error::{BoxError, ToolError};
pub use path::{display_path, guard_external, is_within, normalize, resolve};
pub use repair::validate;
pub use shell::{
    BackgroundJobs, Dialect, JobInfo, JobListener, JobReport, JobState, Segment, ShellKind,
    ShellSpec, ShellTool, split_commands,
};
pub use truncate::{Keep, OutputLimits, Truncated, truncate_output};

/// The names the built-in tools are called by.
pub mod names {
    pub const SHELL: &str = "shell";
    pub const READ: &str = "read";
    pub const WRITE: &str = "write";
    pub const EDIT: &str = "edit";
    pub const PATCH: &str = "patch";
    pub const GLOB: &str = "glob";
    pub const GREP: &str = "grep";
    pub const FETCH: &str = "fetch";
    pub const TODO: &str = "todo";
}

pub fn builtin(config: &ToolsConfig) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(ShellTool::new(
            config.shell.clone(),
            config.output,
            config.jobs.clone(),
        )),
        Arc::new(ReadTool::new(config.read)),
        Arc::new(WriteTool),
        Arc::new(EditTool),
        Arc::new(PatchTool),
        Arc::new(GlobTool::new(config.search)),
        Arc::new(GrepTool::new(config.search)),
        Arc::new(FetchTool::new(config.fetch.clone(), config.output)),
    ];
    if let Some(sink) = &config.todos {
        tools.push(Arc::new(TodoTool::new(Arc::clone(sink))));
    }
    tools
}
