//! What the engine sees of a background shell command: its state, what is known
//! about it, and the report and listener for when it ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;
use systemprompt_identifiers::engine::{JobId, SessionId};

/// Told when a background command ends on its own, times out or is killed by
/// id. Commands removed with [`crate::tools::BackgroundJobs::kill_all`] or
/// [`crate::tools::BackgroundJobs::shutdown`] end silently.
pub trait JobListener: Send + Sync {
    fn finished(&self, report: &JobReport);
}

/// Where a background command stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Exited,
    TimedOut,
    Killed,
}

/// What is known about one background command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobInfo {
    pub id: JobId,
    pub session_id: SessionId,
    pub command: String,
    pub output_path: PathBuf,
    pub started_at: SystemTime,
    pub finished_at: Option<SystemTime>,
    pub state: JobState,
    pub exit_code: Option<i32>,
}

/// A finished background command and the end of its output. `notice` renders it
/// as a short notice for the model, suitable as a synthetic message in the
/// session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobReport {
    pub job: JobInfo,
    pub tail: String,
}

impl JobReport {
    pub fn notice(&self) -> String {
        let outcome = match (self.job.state, self.job.exit_code) {
            (JobState::TimedOut, _) => "timed out and was stopped".to_owned(),
            (JobState::Killed, _) => "was stopped".to_owned(),
            (_, Some(code)) => format!("exited with code {code}"),
            (_, None) => "exited".to_owned(),
        };
        let mut text = format!(
            "Background job {} ({}) {outcome}. Full output: {}",
            self.job.id,
            self.job.command,
            self.job.output_path.to_string_lossy().replace('\\', "/")
        );
        if !self.tail.trim().is_empty() {
            text.push_str("\n\nLast output:\n");
            text.push_str(self.tail.trim_end());
        }
        text
    }
}
