//! Auth-mutating gateway endpoints.
//!
//! POST-style exchanges that hand the bridge a fresh `AuthResponse` or
//! credential bundle: mTLS attestation, browser session swap, PAT exchange,
//! OAuth client provisioning, and per-plugin hook token minting. All four
//! token-bearing variants accept the gateway's plaintext secrets only at
//! call time — callers persist the response immediately. Read-only
//! endpoints live in `fetch`.

use std::time::Instant;

use crate::auth::types::{AuthResponse, MtlsRequest, SessionExchangeRequest};
use crate::gateway::errors::GatewayError;
use crate::gateway::types::{BridgeOAuthClientResponse, HookTokenResponse};
use crate::gateway::{record_span, GatewayClient};

impl GatewayClient {
    pub async fn mtls_exchange(&self, req: &MtlsRequest) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/bridge/mtls", req, "mtls").await
    }

    pub async fn session_exchange(
        &self,
        req: &SessionExchangeRequest,
    ) -> Result<AuthResponse, GatewayError> {
        self.post_json("/v1/auth/bridge/session", req, "session")
            .await
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, pat),
        fields(endpoint = "pat", status, latency_ms)
    )]
    pub async fn pat_exchange(&self, pat: &str) -> Result<AuthResponse, GatewayError> {
        let url = self.url("/v1/auth/bridge/pat");
        let started = Instant::now();
        let resp = self
            .http()
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

    // Plaintext `client_secret` is only returned once per call; callers must persist it to the
    // bridge's secret store immediately.
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
        client_id: &str,
        client_secret: &str,
        plugin_id: &str,
    ) -> Result<HookTokenResponse, GatewayError> {
        let started = Instant::now();
        let form = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("scope", "hook:govern hook:track"),
            ("audience", "hook"),
            ("plugin_id", plugin_id),
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
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "oauth-token",
            });
        }
        resp.json::<HookTokenResponse>()
            .await
            .map_err(|e| GatewayError::HookTokenDecode(Box::new(e)))
    }

    pub(super) async fn post_json<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
        endpoint: &'static str,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url(path);
        let payload = serde_json::to_vec(body)?;
        let started = Instant::now();
        let resp = self
            .http()
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
