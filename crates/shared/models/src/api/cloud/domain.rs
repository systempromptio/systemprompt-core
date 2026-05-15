//! Custom-domain configuration DTOs for cloud tenants.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetCustomDomainRequest {
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DnsInstructions {
    pub record_type: String,
    pub host: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomDomainResponse {
    pub domain: String,
    pub status: String,
    pub verified: bool,
    pub dns_target: String,
    pub dns_instructions: DnsInstructions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}
