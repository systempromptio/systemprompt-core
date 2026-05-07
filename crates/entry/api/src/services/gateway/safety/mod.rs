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

#[async_trait]
pub trait SafetyScanner: Send + Sync {
    fn name(&self) -> &'static str;
    async fn scan_request(&self, req: &CanonicalRequest) -> Vec<Finding>;
    async fn scan_response_final(&self, response: &CanonicalResponse) -> Vec<Finding>;
}

pub use heuristic::HeuristicScanner;
pub use null::NullScanner;
