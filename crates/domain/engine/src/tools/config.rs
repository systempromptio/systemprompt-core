//! Settings for the built-in tools, filled in by the configuration layer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::shell::BackgroundJobs;
use crate::tools::TodoSink;
use crate::truncate::OutputLimits;

/// How shell commands run. `path` replaces the detected shell (a path, or a
/// name looked up on PATH); `capture_bytes` is the output kept in memory per
/// command, beyond which the middle is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellConfig {
    pub path: Option<PathBuf>,
    pub default_timeout: Duration,
    pub max_timeout: Duration,
    pub capture_bytes: usize,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            path: None,
            default_timeout: Duration::from_secs(120),
            max_timeout: Duration::from_secs(600),
            capture_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Limits of the read tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadLimits {
    pub default_lines: usize,
    pub max_line_chars: usize,
    pub max_bytes: usize,
    pub max_attachment_bytes: usize,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            default_lines: 2000,
            max_line_chars: 2000,
            max_bytes: 50 * 1024,
            max_attachment_bytes: 10 * 1024 * 1024,
        }
    }
}

/// Limits of the fetch tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchConfig {
    pub default_timeout: Duration,
    pub max_timeout: Duration,
    pub max_bytes: usize,
    pub user_agent: String,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_timeout: Duration::from_secs(120),
            max_bytes: 5 * 1024 * 1024,
            user_agent: concat!("harness/", env!("CARGO_PKG_VERSION")).to_owned(),
        }
    }
}

/// Limits of glob and grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLimits {
    pub glob_results: usize,
    pub grep_matches: usize,
    pub grep_max_file_bytes: u64,
    pub grep_line_chars: usize,
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            glob_results: 100,
            grep_matches: 100,
            grep_max_file_bytes: 4 * 1024 * 1024,
            grep_line_chars: 400,
        }
    }
}

/// Everything [`crate::builtin`] needs to build the tool set.
///
/// `output` caps what any tool returns before the rest is spilled to a file;
/// `jobs` is the registry background shell commands join, one per engine;
/// without `todos` the todo tool is left out.
#[derive(Clone, Default)]
pub struct ToolsConfig {
    pub shell: ShellConfig,
    pub read: ReadLimits,
    pub fetch: FetchConfig,
    pub search: SearchLimits,
    pub output: OutputLimits,
    pub jobs: BackgroundJobs,
    pub todos: Option<Arc<dyn TodoSink>>,
}

impl fmt::Debug for ToolsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolsConfig")
            .field("shell", &self.shell)
            .field("read", &self.read)
            .field("fetch", &self.fetch)
            .field("search", &self.search)
            .field("output", &self.output)
            .field("jobs", &self.jobs)
            .field("todos", &self.todos.is_some())
            .finish()
    }
}
