use crate::manifest::SignedManifest;
use crate::types::{AuthResponse, MtlsRequest, SessionExchangeRequest};
use std::time::Duration;

pub struct GatewayClient {
    base_url: String,
    agent: ureq::Agent,
}

impl GatewayClient {
    pub fn new(base_url: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        Self { base_url, agent }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn fetch_pubkey(&self) -> Result<String, String> {
        let url = self.url("/v1/cowork/pubkey");
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("pubkey fetch failed: {e}"))?;
        let body: serde_json::Value = resp
            .into_json()
            .map_err(|e| format!("malformed pubkey response: {e}"))?;
        body.get("pubkey")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| "pubkey field missing in response".to_string())
    }

    pub fn fetch_manifest(&self, bearer: &str) -> Result<SignedManifest, String> {
        let url = self.url("/v1/cowork/manifest");
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| format!("manifest fetch failed: {e}"))?;
        resp.into_json::<SignedManifest>()
            .map_err(|e| format!("malformed manifest response: {e}"))
    }

    pub fn fetch_plugin_file(
        &self,
        bearer: &str,
        plugin_id: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, String> {
        if relative_path.contains("..") || relative_path.starts_with('/') {
            return Err(format!("refused unsafe path: {relative_path}"));
        }
        let url = self.url(&format!("/plugins/{plugin_id}/{relative_path}"));
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| format!("plugin fetch {plugin_id}:{relative_path} failed: {e}"))?;
        let mut buf = Vec::with_capacity(4096);
        std::io::copy(&mut resp.into_reader(), &mut buf)
            .map_err(|e| format!("plugin read {plugin_id}:{relative_path} failed: {e}"))?;
        Ok(buf)
    }

    pub fn fetch_whoami(&self, bearer: &str) -> Result<serde_json::Value, String> {
        let url = self.url("/v1/cowork/whoami");
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| format!("whoami fetch failed: {e}"))?;
        resp.into_json::<serde_json::Value>()
            .map_err(|e| format!("malformed whoami response: {e}"))
    }

    pub fn health(&self) -> Result<(), String> {
        let url = self.url("/health");
        self.agent
            .get(&url)
            .call()
            .map_err(|e| format!("health check failed: {e}"))?;
        Ok(())
    }

    pub fn mtls_exchange(&self, req: &MtlsRequest) -> Result<AuthResponse, String> {
        self.post_json(
            "/v1/auth/cowork/mtls",
            serde_json::to_value(req).map_err(|e| e.to_string())?,
        )
    }

    pub fn session_exchange(&self, req: &SessionExchangeRequest) -> Result<AuthResponse, String> {
        self.post_json(
            "/v1/auth/cowork/session",
            serde_json::to_value(req).map_err(|e| e.to_string())?,
        )
    }

    pub fn pat_exchange(&self, pat: &str) -> Result<AuthResponse, String> {
        let url = self.url("/v1/auth/cowork/pat");
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
