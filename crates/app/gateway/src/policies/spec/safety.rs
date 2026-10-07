//! Safety-scanner section of a gateway policy: which scanners run, what they
//! block or redact, and how each scanner's failure and latency are governed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const DEFAULT_SCANNER_TIMEOUT_MS: u64 = 5_000;

/// How far back into a conversation the request-phase scanners look.
///
/// A request carries the whole conversation, so scanning all of it re-reads
/// every earlier turn on every turn: one finding would deny the rest of the
/// conversation, and each turn would persist the same finding again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SafetyHistoryMode {
    #[default]
    Off,
    Audit,
    Block,
}

/// Phrase-list tuning for the builtin `heuristic` scanner.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HeuristicConfig {
    #[serde(default)]
    pub phrases: Option<Vec<String>>,
    #[serde(default)]
    pub extra_phrases: Vec<String>,
    #[serde(default)]
    pub disable_builtin: bool,
}

/// Whether the safety scanners refuse a request or only record what they
/// found.
///
/// `warn` keeps every scanner running and every finding persisted; it only
/// removes the refusal. It exists so the block lists can be calibrated from
/// real traffic — a category that never fires and a category that fires on
/// every developer request look identical until the findings are recorded
/// without blocking.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SafetyMode {
    #[default]
    Enforce,
    Warn,
}

impl SafetyMode {
    #[must_use]
    pub const fn is_warn(self) -> bool {
        matches!(self, Self::Warn)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SafetyConfig {
    #[serde(default)]
    pub mode: SafetyMode,
    #[serde(default)]
    pub scanners: Vec<String>,
    #[serde(default)]
    pub heuristic: HeuristicConfig,
    #[serde(default)]
    pub block_categories: Vec<String>,
    #[serde(default)]
    pub block_response_categories: Vec<String>,
    #[serde(default)]
    pub history: SafetyHistoryMode,
    #[serde(default)]
    pub scanner_settings: BTreeMap<String, ScannerSettings>,
    #[serde(default)]
    pub redact_categories: Vec<String>,
}

impl SafetyConfig {
    #[must_use]
    pub fn redacts(&self, category: &str) -> bool {
        self.redact_categories.iter().any(|c| c == category)
    }

    #[must_use]
    pub fn settings_for(&self, scanner: &str) -> ScannerSettings {
        self.scanner_settings
            .get(scanner)
            .cloned()
            .unwrap_or_default()
    }
}

/// What a scanner that errors or exceeds its timeout means for the request.
///
/// `closed` (the default) treats an unanswered scan as a blocking finding;
/// `open` records the failure and lets the request proceed, for scanners whose
/// availability must not gate inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScannerFailMode {
    Open,
    #[default]
    Closed,
}

/// Per-scanner settings under `safety.scanner_settings.<name>`.
///
/// `fail_mode` and `timeout_ms` are enforced by the gateway for every scanner.
/// `config` is opaque to core: it is handed unchanged to the scanner's factory,
/// so an extension scanner reads its own endpoint, template or credential
/// reference from it without core knowing their shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannerSettings {
    #[serde(default)]
    pub fail_mode: ScannerFailMode,
    #[serde(default = "default_scanner_timeout_ms")]
    pub timeout_ms: u64,
    // JSON: extension-defined scanner configuration, opaque to core and
    // interpreted only by the scanner it names.
    #[serde(default)]
    pub config: BTreeMap<String, serde_json::Value>,
}

impl Default for ScannerSettings {
    fn default() -> Self {
        Self {
            fail_mode: ScannerFailMode::default(),
            timeout_ms: DEFAULT_SCANNER_TIMEOUT_MS,
            config: BTreeMap::new(),
        }
    }
}

impl ScannerSettings {
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

const fn default_scanner_timeout_ms() -> u64 {
    DEFAULT_SCANNER_TIMEOUT_MS
}
