pub mod manifest;
pub mod manifest_version;

use crate::auth::types::{AuthResponse, CoworkProfile, MtlsRequest, SessionExchangeRequest};
use crate::gateway::manifest::SignedManifest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use systemprompt_identifiers::{TenantId, UserId, ValidatedUrl};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("pubkey fetch failed: {0}")]
    PubkeyFetch(Box<reqwest::Error>),
    #[error("malformed pubkey response: {0}")]
    PubkeyDecode(Box<reqwest::Error>),
    #[error("pubkey field missing in response")]
    PubkeyMissing,
    #[error("manifest fetch failed: {0}")]
    ManifestFetch(Box<reqwest::Error>),
    #[error("malformed manifest response: {0}")]
    ManifestDecode(Box<reqwest::Error>),
    #[error("refused unsafe path: {0}")]
    UnsafePath(String),
    #[error("plugin fetch {plugin_id}:{path} failed: {source}")]
    PluginFetch {
        plugin_id: String,
        path: String,
        source: Box<reqwest::Error>,
    },
    #[error("plugin read {plugin_id}:{path} failed: {source}")]
    PluginRead {
        plugin_id: String,
        path: String,
        source: Box<reqwest::Error>,
    },
    #[error("whoami fetch failed: {0}")]
    WhoamiFetch(Box<reqwest::Error>),
    #[error("malformed whoami response: {0}")]
    WhoamiDecode(Box<reqwest::Error>),
    #[error("health check failed: {0}")]
    HealthCheck(Box<reqwest::Error>),
    #[error("cowork profile fetch failed: {0}")]
    ProfileFetch(Box<reqwest::Error>),
    #[error("malformed cowork profile response: {0}")]
    ProfileDecode(Box<reqwest::Error>),
    #[error("gateway PAT request failed: {0}")]
    PatRequest(Box<reqwest::Error>),
    #[error("gateway request failed: {0}")]
    PostRequest(Box<reqwest::Error>),
    #[error("malformed gateway response: {0}")]
    AuthDecode(Box<reqwest::Error>),
    #[error("gateway returned status {status} from {endpoint}")]
    HttpStatus {
        status: reqwest::StatusCode,
        endpoint: &'static str,
    },
    #[error("runtime unavailable: {0}")]
    Runtime(String),
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
}

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .pool_max_idle_per_host(8)
                .tcp_nodelay(true)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

pub struct GatewayClient {
    base_url: ValidatedUrl,
    http: reqwest::Client,
    rt: tokio::runtime::Handle,
}

impl GatewayClient {
    #[must_use]
    pub fn new(base_url: ValidatedUrl) -> Self {
        let http = shared_client();
        let rt = crate::proxy::runtime_handle()
            .or_else(|_| tokio::runtime::Handle::try_current().map_err(|_| ()))
            .unwrap_or_else(|()| {
                tracing::warn!("gateway: no shared runtime available; using current");
                tokio::runtime::Handle::current()
            });
        Self {
            base_url,
            http,
            rt,
        }
    }

    #[must_use]
    pub fn base_url(&self) -> &ValidatedUrl {
        &self.base_url
    }

    #[must_use]
    pub fn base_url_str(&self) -> &str {
        self.base_url.as_str()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path)
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
    }

    #[tracing::instrument(level = "debug", skip(self), fields(endpoint = "pubkey", status, latency_ms))]
    pub async fn fetch_pubkey_async(&self) -> Result<String, GatewayError> {
        #[derive(serde::Deserialize)]
        struct PubkeyResponse {
            #[serde(default)]
            pubkey: Option<String>,
        }
        let url = self.url("/v1/cowork/pubkey");
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::PubkeyFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "pubkey",
            });
        }
        let body: PubkeyResponse = resp
            .json()
            .await
            .map_err(|e| GatewayError::PubkeyDecode(Box::new(e)))?;
        body.pubkey.ok_or(GatewayError::PubkeyMissing)
    }

    pub fn fetch_pubkey(&self) -> Result<String, GatewayError> {
        self.block_on(self.fetch_pubkey_async())
    }

    #[tracing::instrument(level = "debug", skip(self, bearer), fields(endpoint = "manifest", status, latency_ms))]
    pub async fn fetch_manifest_async(
        &self,
        bearer: &str,
    ) -> Result<SignedManifest, GatewayError> {
        let url = self.url("/v1/cowork/manifest");
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::ManifestFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "manifest",
            });
        }
        resp.json::<SignedManifest>()
            .await
            .map_err(|e| GatewayError::ManifestDecode(Box::new(e)))
    }

    pub fn fetch_manifest(&self, bearer: &str) -> Result<SignedManifest, GatewayError> {
        self.block_on(self.fetch_manifest_async(bearer))
    }

    #[tracing::instrument(level = "debug", skip(self, bearer), fields(plugin_id, path, status, latency_ms))]
    pub async fn fetch_plugin_file_async(
        &self,
        bearer: &str,
        plugin_id: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, GatewayError> {
        if relative_path.contains("..") || relative_path.starts_with('/') {
            return Err(GatewayError::UnsafePath(relative_path.to_string()));
        }
        let url = self.url(&format!("/v1/cowork/plugins/{plugin_id}/{relative_path}"));
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::PluginFetch {
                plugin_id: plugin_id.to_string(),
                path: relative_path.to_string(),
                source: Box::new(e),
            })?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "plugin",
            });
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| GatewayError::PluginRead {
                plugin_id: plugin_id.to_string(),
                path: relative_path.to_string(),
                source: Box::new(e),
            })?;
        Ok(bytes.to_vec())
    }

    pub fn fetch_plugin_file(
        &self,
        bearer: &str,
        plugin_id: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, GatewayError> {
        self.block_on(self.fetch_plugin_file_async(bearer, plugin_id, relative_path))
    }

    #[tracing::instrument(level = "debug", skip(self, bearer), fields(endpoint = "whoami", status, latency_ms))]
    pub async fn fetch_whoami_async(&self, bearer: &str) -> Result<WhoamiResponse, GatewayError> {
        let url = self.url("/v1/cowork/whoami");
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::WhoamiFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "whoami",
            });
        }
        resp.json::<WhoamiResponse>()
            .await
            .map_err(|e| GatewayError::WhoamiDecode(Box::new(e)))
    }

    pub fn fetch_whoami(&self, bearer: &str) -> Result<WhoamiResponse, GatewayError> {
        self.block_on(self.fetch_whoami_async(bearer))
    }

    #[tracing::instrument(level = "debug", skip(self), fields(endpoint = "profile", status, latency_ms))]
    pub async fn fetch_cowork_profile_async(&self) -> Result<CoworkProfile, GatewayError> {
        let url = self.url("/v1/cowork/profile");
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::ProfileFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "profile",
            });
        }
        resp.json::<CoworkProfile>()
            .await
            .map_err(|e| GatewayError::ProfileDecode(Box::new(e)))
    }

    pub fn fetch_cowork_profile(&self) -> Result<CoworkProfile, GatewayError> {
        self.block_on(self.fetch_cowork_profile_async())
    }

    #[tracing::instrument(level = "debug", skip(self), fields(endpoint = "health", status, latency_ms))]
    pub async fn health_async(&self) -> Result<(), GatewayError> {
        let url = self.url("/health");
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::HealthCheck(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "health",
            });
        }
        Ok(())
    }

    pub fn health(&self) -> Result<(), GatewayError> {
        self.block_on(self.health_async())
    }

    pub async fn mtls_exchange_async(
        &self,
        req: &MtlsRequest,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json_async("/v1/auth/cowork/mtls", req, "mtls").await
    }

    pub fn mtls_exchange(&self, req: &MtlsRequest) -> Result<AuthResponse, GatewayError> {
        self.block_on(self.mtls_exchange_async(req))
    }

    pub async fn session_exchange_async(
        &self,
        req: &SessionExchangeRequest,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json_async("/v1/auth/cowork/session", req, "session")
            .await
    }

    pub fn session_exchange(
        &self,
        req: &SessionExchangeRequest,
    ) -> Result<AuthResponse, GatewayError> {
        self.block_on(self.session_exchange_async(req))
    }

    #[tracing::instrument(level = "debug", skip(self, pat), fields(endpoint = "pat", status, latency_ms))]
    pub async fn pat_exchange_async(&self, pat: &str) -> Result<AuthResponse, GatewayError> {
        let url = self.url("/v1/auth/cowork/pat");
        let started = Instant::now();
        let resp = self
            .http
            .post(&url)
            .bearer_auth(pat)
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .map_err(|e| GatewayError::PatRequest(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "pat",
            });
        }
        resp.json::<AuthResponse>()
            .await
            .map_err(|e| GatewayError::AuthDecode(Box::new(e)))
    }

    pub fn pat_exchange(&self, pat: &str) -> Result<AuthResponse, GatewayError> {
        self.block_on(self.pat_exchange_async(pat))
    }

    async fn post_json_async<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
        endpoint: &'static str,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url(path);
        let payload = serde_json::to_vec(body)?;
        let started = Instant::now();
        let resp = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint,
            });
        }
        resp.json::<AuthResponse>()
            .await
            .map_err(|e| GatewayError::AuthDecode(Box::new(e)))
    }
}

fn record_span(resp: &reqwest::Response, started: Instant) {
    let span = tracing::Span::current();
    span.record("status", resp.status().as_u16());
    span.record(
        "latency_ms",
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    );
}
