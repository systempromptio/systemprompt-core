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
use systemprompt_wire::canonical::{CanonicalRequest, CanonicalResponse};

use super::spec::SafetyConfig;

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

pub const CATEGORY_SCANNER_FAILURE: &str = "scanner_failure";

#[derive(Debug, Clone)]
pub struct Finding {
    pub phase: &'static str,
    pub severity: Severity,
    pub category: String,
    pub excerpt: Option<String>,
    pub scanner: &'static str,
}

impl Finding {
    #[must_use]
    pub fn scanner_failure(phase: &'static str, scanner: &'static str, error: &ScanError) -> Self {
        Self {
            phase,
            severity: Severity::High,
            category: CATEGORY_SCANNER_FAILURE.to_owned(),
            excerpt: Some(error.to_string()),
            scanner,
        }
    }

    #[must_use]
    pub fn is_scanner_failure(&self) -> bool {
        self.category == CATEGORY_SCANNER_FAILURE
    }
}

/// Why a [`SafetyScanner`] could not produce a verdict. A failed scan is never
/// read as clean: callers record it as a [`CATEGORY_SCANNER_FAILURE`] finding.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("safety scanner {scanner} failed: {reason}")]
    Failed {
        scanner: &'static str,
        reason: String,
    },
}

/// Built by the scanner registry as `Arc<dyn SafetyScanner>` and fanned out
/// per request; `#[async_trait]` keeps it object-safe.
#[async_trait]
pub trait SafetyScanner: Send + Sync {
    fn name(&self) -> &'static str;

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError>;

    async fn scan_request_history(
        &self,
        _req: &CanonicalRequest,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }

    async fn scan_response_final(
        &self,
        response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError>;
}

/// Constructs a scanner for one policy's [`SafetyConfig`], so per-policy
/// configuration (such as the heuristic phrase list) applies at scan time.
pub trait ScannerFactory: Send + Sync {
    fn create(&self, safety: &SafetyConfig) -> Arc<dyn SafetyScanner>;
}

impl<F> ScannerFactory for F
where
    F: Fn(&SafetyConfig) -> Arc<dyn SafetyScanner> + Send + Sync,
{
    fn create(&self, safety: &SafetyConfig) -> Arc<dyn SafetyScanner> {
        self(safety)
    }
}

/// Compile-time registration of a [`SafetyScanner`] implementation.
///
/// The gateway's scanner registry seeds its built-ins, then folds in every
/// `inventory`-collected registration. A registration whose `name` collides
/// with a built-in is rejected at registry build time.
#[derive(Debug, Clone, Copy)]
pub struct SafetyScannerRegistration {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn SafetyScanner>,
}

inventory::collect!(SafetyScannerRegistration);

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
