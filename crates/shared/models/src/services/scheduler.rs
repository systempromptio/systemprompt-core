//! Scheduler job configuration and the built-in job set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobConfig {
    #[serde(default)]
    pub extension: Option<String>,
    pub name: String,
    #[serde(default)]
    pub owner: Option<UserId>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub schedule: Option<String>,
}

const fn default_true() -> bool {
    true
}

impl JobConfig {
    /// A job with no explicit `owner` runs as the profile `system_admin`,
    /// resolved per-environment at scheduler start. Set one with
    /// [`Self::with_owner`] only for a job that must run as a specific user.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            extension: None,
            name: name.into(),
            owner: None,
            enabled: true,
            schedule: None,
        }
    }

    #[must_use]
    pub fn with_owner(mut self, owner: UserId) -> Self {
        self.owner = Some(owner);
        self
    }

    #[must_use]
    pub fn with_extension(mut self, extension: impl Into<String>) -> Self {
        self.extension = Some(extension.into());
        self
    }

    #[must_use]
    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }

    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub jobs: Vec<JobConfig>,
    #[serde(default = "default_bootstrap_jobs")]
    pub bootstrap_jobs: Vec<String>,
    #[serde(default = "default_true")]
    pub distributed_lock: bool,
}

fn default_bootstrap_jobs() -> Vec<String> {
    vec![
        "database_cleanup".to_owned(),
        "cleanup_inactive_sessions".to_owned(),
    ]
}

impl SchedulerConfig {
    /// The built-in core job set. The four cleanup jobs
    /// (`cleanup_anonymous_users`, `cleanup_empty_contexts`,
    /// `cleanup_inactive_sessions`, `database_cleanup`) have no human
    /// originator, so they carry no explicit `owner` and run as the profile
    /// `system_admin` resolved per-environment at scheduler start.
    #[must_use]
    pub fn with_system_admin() -> Self {
        Self {
            enabled: true,
            jobs: vec![
                JobConfig::new("cleanup_anonymous_users")
                    .with_extension("core")
                    .with_schedule("0 0 3 * * *"),
                JobConfig::new("cleanup_empty_contexts")
                    .with_extension("core")
                    .with_schedule("0 0 * * * *"),
                JobConfig::new("cleanup_inactive_sessions")
                    .with_extension("core")
                    .with_schedule("0 0 * * * *"),
                JobConfig::new("database_cleanup")
                    .with_extension("core")
                    .with_schedule("0 0 4 * * *"),
            ],
            bootstrap_jobs: default_bootstrap_jobs(),
            distributed_lock: true,
        }
    }
}
