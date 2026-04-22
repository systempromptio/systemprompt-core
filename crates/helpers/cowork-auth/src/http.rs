use crate::types::{AuthRequest, AuthResponse};
use std::time::Duration;

pub struct GatewayClient {
    base_url: String,
    agent: ureq::Agent,
}

impl GatewayClient {
    pub fn new(base_url: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .build();
        Self { base_url, agent }
    }

    pub fn mtls_exchange(&self, req: &AuthRequest) -> Result<AuthResponse, String> {
        self.post_json(
            "/v1/gateway/auth/cowork/mtls",
            serde_json::to_value(req).map_err(|e| e.to_string())?,
        )
    }

    pub fn pat_exchange(&self, pat: &str) -> Result<AuthResponse, String> {
        let url = self.url("/v1/gateway/auth/cowork/pat");
        let resp = self
            .agent
            .post(&url)
            .set("authorization", &format!("Bearer {pat}"))
            .set("content-type", "application/json")
            .send_string("{}")
            .map_err(|e| format!("gateway PAT request failed: {e}"))?;
        resp.into_json::<AuthResponse>()
            .map_err(|e| format!("malformed gateway response: {e}"))
    }

    fn post_json(&self, path: &str, body: serde_json::Value) -> Result<AuthResponse, String> {
        let url = self.url(path);
        let resp = self
            .agent
            .post(&url)
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| format!("gateway request failed: {e}"))?;
        resp.into_json::<AuthResponse>()
            .map_err(|e| format!("malformed gateway response: {e}"))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}
