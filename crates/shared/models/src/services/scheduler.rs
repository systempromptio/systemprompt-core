use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobConfig {
    #[serde(default)]
    pub extension: Option<String>,
    pub name: String,
    pub owner: UserId,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub schedule: Option<String>,
}

const fn default_true() -> bool {
    true
}

impl JobConfig {
    #[must_use]
    pub fn new(name: impl Into<String>, owner: UserId) -> Self {
        Self {
            extension: None,
            name: name.into(),
            owner,
            enabled: true,
            schedule: None,
        }
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
        "database_cleanup".to_string(),
        "cleanup_inactive_sessions".to_string(),
    ]
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        // Why: bootstrap scheduler jobs (cleanup, retention) have no human originator,
        // so they declare the platform owner (admin until delegated) as their actor.
        // `crates/app/scheduler/src/services/scheduling/mod.rs::resolve_owners`
        // validates at startup that this name resolves to an active user; the
        // platform refuses to start otherwise. This is the only sanctioned
        // UserId::admin() call site outside the bootstrap CLI command and the
        // actor module.
        let owner = UserId::admin();
        Self {
            enabled: true,
            jobs: vec![
                JobConfig::new("cleanup_anonymous_users", owner.clone())
                    .with_extension("core")
                    .with_schedule("0 0 3 * * *"),
                JobConfig::new("cleanup_empty_contexts", owner.clone())
                    .with_extension("core")
                    .with_schedule("0 0 * * * *"),
                JobConfig::new("cleanup_inactive_sessions", owner.clone())
                    .with_extension("core")
                    .with_schedule("0 0 * * * *"),
                JobConfig::new("database_cleanup", owner)
                    .with_extension("core")
                    .with_schedule("0 0 4 * * *"),
            ],
            bootstrap_jobs: default_bootstrap_jobs(),
            distributed_lock: true,
        }
    }
}
