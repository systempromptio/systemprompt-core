use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    #[serde(default)]
    pub extension: Option<String>,
    pub name: String,
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
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            extension: None,
            name: name.into(),
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
}

fn default_bootstrap_jobs() -> Vec<String> {
    vec![
        "database_cleanup".to_string(),
        "cleanup_inactive_sessions".to_string(),
    ]
}

impl Default for SchedulerConfig {
    fn default() -> Self {
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
                JobConfig::new("publish_content")
                    .with_extension("content")
                    .with_schedule("0 */30 * * * *"),
            ],
            bootstrap_jobs: default_bootstrap_jobs(),
        }
    }
}
