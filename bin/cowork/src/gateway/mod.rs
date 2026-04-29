pub mod manifest;

use crate::auth::types::{AuthResponse, CoworkProfile, MtlsRequest, SessionExchangeRequest};
use crate::gateway::manifest::SignedManifest;
use std::time::Duration;
use systemprompt_identifiers::ValidatedUrl;

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("pubkey fetch failed: {0}")]
    PubkeyFetch(Box<ureq::Error>),
    #[error("malformed pubkey response: {0}")]
    PubkeyDecode(std::io::Error),
    #[error("pubkey field missing in response")]
    PubkeyMissing,
    #[error("manifest fetch failed: {0}")]
    ManifestFetch(Box<ureq::Error>),
    #[error("malformed manifest response: {0}")]
    ManifestDecode(std::io::Error),
    #[error("refused unsafe path: {0}")]
    UnsafePath(String),
    #[error("plugin fetch {plugin_id}:{path} failed: {source}")]
    PluginFetch {
        plugin_id: String,
        path: String,
        source: Box<ureq::Error>,
    },
    #[error("plugin read {plugin_id}:{path} failed: {source}")]
    PluginRead {
        plugin_id: String,
        path: String,
        source: std::io::Error,
    },
    #[error("whoami fetch failed: {0}")]
    WhoamiFetch(Box<ureq::Error>),
    #[error("malformed whoami response: {0}")]
    WhoamiDecode(std::io::Error),
    #[error("health check failed: {0}")]
    HealthCheck(Box<ureq::Error>),
    #[error("cowork profile fetch failed: {0}")]
    ProfileFetch(Box<ureq::Error>),
    #[error("malformed cowork profile response: {0}")]
    ProfileDecode(std::io::Error),
    #[error("gateway PAT request failed: {0}")]
    PatRequest(Box<ureq::Error>),
    #[error("gateway request failed: {0}")]
    PostRequest(Box<ureq::Error>),
    #[error("malformed gateway response: {0}")]
    AuthDecode(std::io::Error),
    #[error("{0}")]
    Serialize(String),
}

pub struct GatewayClient {
    base_url: ValidatedUrl,
    agent: ureq::Agent,
}

impl GatewayClient {
    pub fn new(base_url: ValidatedUrl) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .build();
        Self { base_url, agent }
    }

    pub fn base_url(&self) -> &ValidatedUrl {
        &self.base_url
    }

    pub fn base_url_str(&self) -> &str {
        self.base_url.as_str()
    }

    pub fn fetch_pubkey(&self) -> Result<String, GatewayError> {
        #[derive(serde::Deserialize)]
        struct PubkeyResponse {
            #[serde(default)]
            pubkey: Option<String>,
        }

        let url = self.url("/v1/cowork/pubkey");
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| GatewayError::PubkeyFetch(Box::new(e)))?;
        let body: PubkeyResponse = resp.into_json().map_err(GatewayError::PubkeyDecode)?;
        body.pubkey.ok_or(GatewayError::PubkeyMissing)
    }

    pub fn fetch_manifest(&self, bearer: &str) -> Result<SignedManifest, GatewayError> {
        let url = self.url("/v1/cowork/manifest");
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| GatewayError::ManifestFetch(Box::new(e)))?;
        resp.into_json::<SignedManifest>()
            .map_err(GatewayError::ManifestDecode)
    }

    pub fn fetch_plugin_file(
        &self,
        bearer: &str,
        plugin_id: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, GatewayError> {
        if relative_path.contains("..") || relative_path.starts_with('/') {
            return Err(GatewayError::UnsafePath(relative_path.to_string()));
        }
        let url = self.url(&format!("/v1/cowork/plugins/{plugin_id}/{relative_path}"));
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| GatewayError::PluginFetch {
                plugin_id: plugin_id.to_string(),
                path: relative_path.to_string(),
                source: Box::new(e),
            })?;
        let mut buf = Vec::with_capacity(4096);
        std::io::copy(&mut resp.into_reader(), &mut buf).map_err(|e| GatewayError::PluginRead {
            plugin_id: plugin_id.to_string(),
            path: relative_path.to_string(),
            source: e,
        })?;
        Ok(buf)
    }

    // JSON: protocol boundary — gateway response shape is opaque to cowork; passed
    // through to CLI for pretty-printing.
    pub fn fetch_whoami(&self, bearer: &str) -> Result<serde_json::Value, GatewayError> {
        let url = self.url("/v1/cowork/whoami");
        let resp = self
            .agent
            .get(&url)
            .set("authorization", &format!("Bearer {bearer}"))
            .call()
            .map_err(|e| GatewayError::WhoamiFetch(Box::new(e)))?;
        resp.into_json::<serde_json::Value>()
            .map_err(GatewayError::WhoamiDecode)
    }

    pub fn fetch_cowork_profile(&self) -> Result<CoworkProfile, GatewayError> {
        let url = self.url("/v1/cowork/profile");
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| GatewayError::ProfileFetch(Box::new(e)))?;
        resp.into_json::<CoworkProfile>()
            .map_err(GatewayError::ProfileDecode)
    }

    pub fn health(&self) -> Result<(), GatewayError> {
        let url = self.url("/health");
        self.agent
            .get(&url)
            .call()
            .map_err(|e| GatewayError::HealthCheck(Box::new(e)))?;
        Ok(())
    }

    pub fn mtls_exchange(&self, req: &MtlsRequest) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/cowork/mtls", req)
    }

    pub fn session_exchange(
        &self,
        req: &SessionExchangeRequest,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/cowork/session", req)
    }

    pub fn pat_exchange(&self, pat: &str) -> Result<AuthResponse, GatewayError> {
        let url = self.url("/v1/auth/cowork/pat");
        let resp = self
            .agent
            .post(&url)
            .set("authorization", &format!("Bearer {pat}"))
            .set("content-type", "application/json")
            .send_string("{}")
            .map_err(|e| GatewayError::PatRequest(Box::new(e)))?;
        resp.into_json::<AuthResponse>()
            .map_err(GatewayError::AuthDecode)
    }

    fn post_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url(path);
        let payload =
            serde_json::to_value(body).map_err(|e| GatewayError::Serialize(e.to_string()))?;
        let resp = self
            .agent
            .post(&url)
            .set("content-type", "application/json")
            .send_json(payload)
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        resp.into_json::<AuthResponse>()
            .map_err(GatewayError::AuthDecode)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path)
    }
}
