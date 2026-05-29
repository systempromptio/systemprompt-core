//! Content-safety scanning of gateway requests and responses.
//!
//! The [`SafetyScanner`] trait inspects a canonical request or final response
//! and returns [`Finding`]s graded by [`Severity`]. Scanners are selected by
//! policy: [`HeuristicScanner`] applies pattern-based checks, [`NullScanner`]
//! is the no-op used when scanning is disabled.

pub mod heuristic;
pub mod null;

use async_trait::async_trait;

use super::protocol::canonical::CanonicalRequest;
use super::protocol::canonical_response::CanonicalResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

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
    async fn scan_request(&self, req: &CanonicalRequest) -> Vec<Finding>;
    async fn scan_response_final(&self, response: &CanonicalResponse) -> Vec<Finding>;
}

pub use heuristic::HeuristicScanner;
pub use null::NullScanner;
