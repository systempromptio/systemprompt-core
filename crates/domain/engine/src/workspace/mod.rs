//! Per-directory services the engine runs around file changes.
//!
//! - [`SnapshotStore`]: a content-addressed shadow store per worktree, kept
//!   under the injected data directory, used to diff and roll back what an
//!   agent changed.
//! - [`unified_diff`], [`file_diff`], [`DiffSummary`]: patches and line
//!   statistics.
//! - [`Formatter`]: runs the first formatter available for a file, from a
//!   built-in table and from configured specs.
//! - [`FileWatcher`]: debounced recursive watching with ignore rules, on a
//!   broadcast channel.
//! - [`VcsInfo`]: branch, head commit and a dirty flag read from `.git` without
//!   spawning git.
//! - [`Workspace`]: one directory's bundle of the above, cheap to clone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod diff;
mod format;
mod paths;
mod services;
mod snapshot;
mod vcs;
mod watch;

pub use diff::{DiffSummary, FileDiff, FileStatus, LineStats, file_diff, line_stats, unified_diff};
pub use format::{
    FormatError, Formatter, FormatterConfig, FormatterSpec, ResolvedFormatter, builtin_formatters,
};
pub use services::{PruneSchedule, Workspace, WorkspaceOptions};
pub use snapshot::{
    DiffTarget, PrunePolicy, PruneReport, SnapshotError, SnapshotId, SnapshotInfo, SnapshotOptions,
    SnapshotStore,
};
pub use vcs::{VcsError, VcsInfo, VcsStatus};
pub use watch::{FileEvent, FileEventKind, FileWatcher, WatchError, WatchOptions};
