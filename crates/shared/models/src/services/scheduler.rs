//! Scheduler job configuration and the built-in job set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
pub use systemprompt_provider_contracts::JobScope;

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
    #[serde(default)]
    pub enforce: bool,
    #[serde(default)]
    pub parameters: HashMap<String, String>,
    #[serde(default)]
    pub scope: Option<JobScope>,
}

const fn default_true() -> bool {
    true
}

impl JobConfig {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            extension: None,
            name: name.into(),
            owner: None,
            enabled: true,
            schedule: None,
            enforce: false,
            parameters: HashMap::new(),
            scope: None,
        }
    }

    #[must_use]
    pub const fn with_enforce(mut self) -> Self {
        self.enforce = true;
        self
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
    pub fn with_parameters(mut self, parameters: HashMap<String, String>) -> Self {
        self.parameters = parameters;
        self
    }

    #[must_use]
    pub const fn with_scope(mut self, scope: JobScope) -> Self {
        self.scope = Some(scope);
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
    vec!["cleanup_inactive_sessions".to_owned()]
}

impl SchedulerConfig {
    #[must_use]
    pub fn with_system_admin() -> Self {
        Self {
            enabled: true,
            jobs: vec![
                JobConfig::new("cleanup_anonymous_users")
                    .with_extension("core")
                    .with_schedule("0 0 3 * * *")
                    .with_enforce(),
                JobConfig::new("cleanup_empty_contexts")
                    .with_extension("core")
                    .with_schedule("0 0 * * * *")
                    .with_enforce(),
                JobConfig::new("cleanup_inactive_sessions")
                    .with_extension("core")
                    .with_schedule("0 0 * * * *"),
                JobConfig::new("database_cleanup")
                    .with_extension("core")
                    .with_schedule("0 0 4 * * *")
                    .with_enforce(),
            ],
            bootstrap_jobs: default_bootstrap_jobs(),
            distributed_lock: true,
        }
    }
}
