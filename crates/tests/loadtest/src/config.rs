use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScenarioId(pub String);

impl ScenarioId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for ScenarioId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Stage {
    pub duration: Duration,
    pub target_users: usize,
}

/// Pass/fail SLO for a scenario. A profile sets a fast-path default and may
/// override it per scenario — different scenarios do different amounts of work,
/// so a single per-profile bar would either be too loose for fast paths or too
/// tight for slow ones.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Thresholds {
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub max_error_rate: f64,
}

impl Thresholds {
    /// Latency budget for scenarios that proxy to an upstream model and write
    /// the governance audit spine (`gateway-inference`, `send-message`). They
    /// do materially more work per request than a fast read or an early policy
    /// deny — an upstream round-trip plus several audit-row writes — so their
    /// latency ceiling is scaled up while the error budget is left untouched.
    fn inference(&self) -> Self {
        Self {
            p95_ms: self.p95_ms * INFERENCE_LATENCY_FACTOR / 2,
            p99_ms: self.p99_ms * INFERENCE_LATENCY_FACTOR / 2,
            max_error_rate: self.max_error_rate,
        }
    }
}

/// The inference path's latency budget as a fraction (×N/2) of the fast-path
/// budget. 5/2 == 2.5×: an upstream round-trip plus audit-spine writes is
/// budgeted at two-and-a-half times a fast read/deny.
const INFERENCE_LATENCY_FACTOR: u64 = 5;

/// Scenarios whose work profile is "proxy upstream + write the audit spine",
/// and therefore carry the relaxed [`Thresholds::inference`] budget in every
/// profile rather than the fast-path default.
const INFERENCE_SCENARIOS: [&str; 2] = ["gateway-inference", "send-message"];

#[derive(Debug, Clone)]
pub struct LoadConfig {
    pub base_url: String,
    pub stages: Vec<Stage>,
    /// SLO applied to any scenario without a more specific entry in
    /// [`Self::scenario_thresholds`].
    pub default_thresholds: Thresholds,
    /// Per-scenario SLO overrides, keyed by scenario name.
    pub scenario_thresholds: BTreeMap<String, Thresholds>,
    pub token: Option<String>,
}

impl LoadConfig {
    /// Build a profile from its stages and fast-path default SLO, deriving the
    /// per-scenario overrides (currently the inference-path budget) so every
    /// profile gets consistent, scenario-aware thresholds.
    fn new(
        base_url: String,
        token: Option<String>,
        stages: Vec<Stage>,
        default_thresholds: Thresholds,
    ) -> Self {
        let scenario_thresholds = INFERENCE_SCENARIOS
            .iter()
            .map(|name| ((*name).to_string(), default_thresholds.inference()))
            .collect();
        Self {
            base_url,
            token,
            stages,
            default_thresholds,
            scenario_thresholds,
        }
    }

    /// The SLO that governs `scenario` — its specific override if one exists,
    /// otherwise the profile's fast-path default.
    pub fn thresholds_for(&self, scenario: &str) -> &Thresholds {
        self.scenario_thresholds
            .get(scenario)
            .unwrap_or(&self.default_thresholds)
    }

    pub fn ci(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(10),
                    target_users: 10,
                },
                Stage {
                    duration: Duration::from_secs(15),
                    target_users: 10,
                },
                Stage {
                    duration: Duration::from_secs(5),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 500,
                p99_ms: 1000,
                max_error_rate: 0.05,
            },
        )
    }

    pub fn airgap(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(20),
                    target_users: 20,
                },
                Stage {
                    duration: Duration::from_secs(60),
                    target_users: 20,
                },
                Stage {
                    duration: Duration::from_secs(20),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 300,
                p99_ms: 600,
                max_error_rate: 0.005,
            },
        )
    }

    pub fn scaled(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(60),
                    target_users: 100,
                },
                Stage {
                    duration: Duration::from_secs(120),
                    target_users: 250,
                },
                Stage {
                    duration: Duration::from_secs(120),
                    target_users: 500,
                },
                Stage {
                    duration: Duration::from_secs(180),
                    target_users: 1000,
                },
                Stage {
                    duration: Duration::from_secs(30),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 500,
                p99_ms: 1000,
                max_error_rate: 0.02,
            },
        )
    }

    pub fn soak(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(60),
                    target_users: 20,
                },
                Stage {
                    duration: Duration::from_secs(3600),
                    target_users: 20,
                },
                Stage {
                    duration: Duration::from_secs(30),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 250,
                p99_ms: 400,
                max_error_rate: 0.001,
            },
        )
    }

    pub fn spike(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(60),
                    target_users: 50,
                },
                Stage {
                    duration: Duration::from_secs(30),
                    target_users: 800,
                },
                Stage {
                    duration: Duration::from_secs(90),
                    target_users: 50,
                },
                Stage {
                    duration: Duration::from_secs(20),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 2000,
                p99_ms: 5000,
                max_error_rate: 0.10,
            },
        )
    }

    pub fn default_profile(base_url: String, token: Option<String>) -> Self {
        Self::new(
            base_url,
            token,
            vec![
                Stage {
                    duration: Duration::from_secs(30),
                    target_users: 50,
                },
                Stage {
                    duration: Duration::from_secs(120),
                    target_users: 100,
                },
                Stage {
                    duration: Duration::from_secs(60),
                    target_users: 100,
                },
                Stage {
                    duration: Duration::from_secs(30),
                    target_users: 0,
                },
            ],
            Thresholds {
                p95_ms: 300,
                p99_ms: 500,
                max_error_rate: 0.01,
            },
        )
    }
}
