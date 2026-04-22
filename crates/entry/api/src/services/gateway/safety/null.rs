use async_trait::async_trait;

use super::{Finding, SafetyScanner};
use crate::services::gateway::models::AnthropicGatewayRequest;

#[derive(Debug, Clone, Copy, Default)]
pub struct NullScanner;

#[async_trait]
impl SafetyScanner for NullScanner {
    fn name(&self) -> &'static str {
        "null"
    }
    async fn scan_request(&self, _req: &AnthropicGatewayRequest) -> Vec<Finding> {
        Vec::new()
    }
    async fn scan_response_final(&self, _body: &[u8]) -> Vec<Finding> {
        Vec::new()
    }
}
