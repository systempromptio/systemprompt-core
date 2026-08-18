//! Content-safety scanning of gateway requests and responses.
//!
//! The [`SafetyScanner`] trait inspects a canonical request or final response
//! and returns [`Finding`]s graded by [`Severity`]. Scanners are selected by
//! policy (`SafetyConfig::scanners`) and resolved against a registry: the
//! built-in [`HeuristicScanner`] applies pattern-based checks, [`NullScanner`]
//! is the no-op used when scanning is disabled.
//!
//! Extensions contribute additional scanners the same way they contribute
//! gateway upstreams or marketplace filters — by submitting a
//! [`SafetyScannerRegistration`] through the
//! [`register_safety_scanner!`](crate::register_safety_scanner) macro, which
//! the consuming layer collects via `inventory::iter`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod heuristic;
mod null;

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_models::wire::canonical::{CanonicalRequest, CanonicalResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

pub const PHASE_REQUEST: &str = "request";

pub const PHASE_REQUEST_HISTORY: &str = "request_history";

pub const PHASE_RESPONSE: &str = "response";

#[derive(Debug, Clone)]
pub struct Finding {
    pub phase: &'static str,
    pub severity: Severity,
    pub category: String,
    pub excerpt: Option<String>,
    pub scanner: &'static str,
}

// Why: #[async_trait] is required — scanners are selected by policy and held as
// trait objects, so the trait must stay dyn-compatible.
#[async_trait]
pub trait SafetyScanner: Send + Sync {
    fn name(&self) -> &'static str;

    /// Judges the newest turn of `req`. Findings belong at [`PHASE_REQUEST`]
    /// and deny the request when their category is blocked by policy.
    ///
    /// A request carries the whole conversation, so a scanner that reads all of
    /// it here denies every later turn once a single finding lands. Earlier
    /// turns belong in [`Self::scan_request_history`].
    async fn scan_request(&self, req: &CanonicalRequest) -> Vec<Finding>;

    /// Judges the turns preceding the newest one. Findings belong at
    /// [`PHASE_REQUEST_HISTORY`] and are recorded without denying the request
    /// unless policy sets `SafetyConfig::history` to `block`.
    async fn scan_request_history(&self, _req: &CanonicalRequest) -> Vec<Finding> {
        Vec::new()
    }

    async fn scan_response_final(&self, response: &CanonicalResponse) -> Vec<Finding>;
}

/// Compile-time registration of a [`SafetyScanner`] implementation.
///
/// The gateway's scanner registry seeds its built-ins, then folds in every
/// `inventory`-collected registration. A registration whose `name` collides
/// with a built-in shadows it (with a warning at registry build time).
#[derive(Debug, Clone, Copy)]
pub struct SafetyScannerRegistration {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn SafetyScanner>,
}

inventory::collect!(SafetyScannerRegistration);

/// Register a [`SafetyScanner`] implementation with the gateway.
///
/// ```ignore
/// use systemprompt_ai::register_safety_scanner;
/// register_safety_scanner!(SecretsScanner::new, name = "secrets");
/// ```
///
/// `$factory` is any `fn() -> Arc<dyn SafetyScanner>`.
#[macro_export]
macro_rules! register_safety_scanner {
    ($factory:expr, name = $name:expr $(,)?) => {
        ::inventory::submit! {
            $crate::SafetyScannerRegistration {
                name: $name,
                factory: || ::std::sync::Arc::new($factory()),
            }
        }
    };
}

pub use heuristic::{HeuristicScanner, effective_phrases};
pub use null::NullScanner;
