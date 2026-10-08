//! No-op safety scanner for policies with scanning disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_wire::canonical::{CanonicalRequest, CanonicalResponse};

use super::{Finding, SafetyScanner, ScanError};

#[derive(Debug, Clone, Copy, Default)]
pub struct NullScanner;

#[async_trait]
impl SafetyScanner for NullScanner {
    fn name(&self) -> &'static str {
        "null"
    }
    async fn scan_request(&self, _req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}
