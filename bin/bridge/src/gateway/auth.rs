//! Auth-mutating gateway endpoints: mTLS attestation, session swap, PAT
//! exchange, OAuth client provisioning, and per-plugin hook token minting.

use std::time::Instant;

use systemprompt_identifiers::{ClientId, PluginId, SessionId, headers as sp_headers};

use crate::auth::types::{AuthResponse, MtlsRequest, SessionExchangeRequest};
use crate::gateway::errors::GatewayError;
use crate::gateway::types::{BridgeOAuthClientResponse, HookTokenResponse};
use crate::gateway::{GatewayClient, record_span};

impl GatewayClient {
    pub async fn mtls_exchange(
        &self,
        req: &MtlsRequest,
        session_id: &SessionId,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/bridge/mtls", req, "mtls", session_id)
            .await
    }

    pub async fn session_exchange(
        &self,
        req: &SessionExchangeRequest,
        session_id: &SessionId,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/bridge/session", req, "session", session_id)
            .await
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, pat),
        fields(endpoint = "pat", status, latency_ms)
    )]
    pub async fn pat_exchange(
        &self,
        pat: &str,
        session_id: &SessionId,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url("/v1/auth/bridge/pat");
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(pat)
            .header("content-type", "application/json")
            .header(sp_headers::SESSION_ID, session_id.as_str())
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

    // Plaintext `client_secret` is returned once per call; persist it immediately.
    #[tracing::instrument(
        level = "debug",
        skip(self, pat),
        fields(endpoint = "oauth-client", status, latency_ms)
    )]
    pub async fn provision_oauth_client(
        &self,
        pat: &str,
    ) -> Result<BridgeOAuthClientResponse, GatewayError> {
        let url = self.url("/v1/auth/bridge/oauth-client");
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(pat)
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .map_err(|e| GatewayError::OAuthClientRequest(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "oauth-client",
            });
        }
        resp.json::<BridgeOAuthClientResponse>()
            .await
            .map_err(|e| GatewayError::OAuthClientDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, client_id, client_secret),
        fields(plugin_id, status, latency_ms)
    )]
    pub async fn mint_plugin_hook_token(
        &self,
        token_endpoint: &str,
        client_id: &ClientId,
        client_secret: &str,
        plugin_id: &PluginId,
    ) -> Result<HookTokenResponse, GatewayError> {
        let started = Instant::now();
        let form = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret),
            ("scope", "hook:govern hook:track"),
            ("audience", "hook"),
            ("plugin_id", plugin_id.as_str()),
        ];
        let resp = self
            .http()
            .post(token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| GatewayError::HookTokenRequest(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GatewayError::HookTokenRejected { status, body });
        }
        resp.json::<HookTokenResponse>()
            .await
            .map_err(|e| GatewayError::HookTokenDecode(Box::new(e)))
    }

    pub(super) async fn post_json<T: serde::Serialize + Sync>(
        &self,
        path: &str,
        body: &T,
        endpoint: &'static str,
        session_id: &SessionId,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url(path);
        let payload = serde_json::to_vec(body)?;
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .header("content-type", "application/json")
            .header(sp_headers::SESSION_ID, session_id.as_str())
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
