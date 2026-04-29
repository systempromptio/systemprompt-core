use crate::auth::secret::Secret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoworkProfile {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsRequest {
    pub device_cert_fingerprint: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExchangeRequest {
    pub code: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: Secret,
    pub ttl: u64,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperOutput {
    pub token: Secret,
    pub ttl: u64,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl From<AuthResponse> for HelperOutput {
    fn from(r: AuthResponse) -> Self {
        Self {
            token: r.token,
            ttl: r.ttl,
            headers: r.headers,
        }
    }
}
