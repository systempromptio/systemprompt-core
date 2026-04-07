use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Stage {
    pub duration: Duration,
    pub target_users: usize,
}

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub max_error_rate: f64,
}

#[derive(Debug, Clone)]
pub struct LoadConfig {
    pub base_url: String,
    pub stages: Vec<Stage>,
    pub thresholds: Thresholds,
    pub token: Option<String>,
}

impl LoadConfig {
    pub fn ci(base_url: String, token: Option<String>) -> Self {
        Self {
            base_url,
            token,
            stages: vec![
                Stage { duration: Duration::from_secs(10), target_users: 10 },
                Stage { duration: Duration::from_secs(15), target_users: 10 },
                Stage { duration: Duration::from_secs(5), target_users: 0 },
            ],
            thresholds: Thresholds {
                p95_ms: 500,
                p99_ms: 1000,
                max_error_rate: 0.05,
            },
        }
    }

    pub fn default_profile(base_url: String, token: Option<String>) -> Self {
        Self {
            base_url,
            token,
            stages: vec![
                Stage { duration: Duration::from_secs(30), target_users: 50 },
                Stage { duration: Duration::from_secs(120), target_users: 100 },
                Stage { duration: Duration::from_secs(60), target_users: 100 },
                Stage { duration: Duration::from_secs(30), target_users: 0 },
            ],
            thresholds: Thresholds {
                p95_ms: 300,
                p99_ms: 500,
                max_error_rate: 0.01,
            },
        }
    }
}
