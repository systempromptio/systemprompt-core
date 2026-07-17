//! No-op safety scanner for policies with scanning disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_models::wire::canonical::{CanonicalRequest, CanonicalResponse};

use super::{Finding, SafetyScanner};

#[derive(Debug, Clone, Copy, Default)]
pub struct NullScanner;

#[async_trait]
impl SafetyScanner for NullScanner {
    fn name(&self) -> &'static str {
        "null"
    }
    async fn scan_request(&self, _req: &CanonicalRequest) -> Vec<Finding> {
        Vec::new()
    }
    async fn scan_response_final(&self, _response: &CanonicalResponse) -> Vec<Finding> {
        Vec::new()
    }
}
