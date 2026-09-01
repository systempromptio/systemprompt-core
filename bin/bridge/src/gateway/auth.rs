//! Auth-mutating gateway endpoints: mTLS attestation, session swap, PAT
//! exchange, OAuth client provisioning, and per-plugin hook token minting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::{ClientId, PluginId, SessionId, headers as sp_headers};

use crate::gateway::errors::GatewayError;
use crate::gateway::types::{
    AuthResponse, BridgeOAuthClientResponse, DevicePatResponse, HookTokenResponse, MtlsRequest,
    SessionExchangeRequest, SessionPatRequest,
};
use crate::gateway::{GatewayClient, record_span};
use crate::ids::{BearerToken, PatToken};

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

    pub async fn session_pat_exchange(
        &self,
        req: &SessionPatRequest,
        session_id: &SessionId,
    ) -> Result<PatToken, GatewayError> {
        let resp: DevicePatResponse = self
            .post_json(
                "/v1/auth/bridge/session-pat",
                req,
                "session-pat",
                session_id,
            )
            .await?;
        Ok(PatToken::new(resp.pat))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, pat),
        fields(endpoint = "pat", status, latency_ms)
    )]
    pub async fn pat_exchange(
        &self,
        pat: &PatToken,
        session_id: &SessionId,
    ) -> Result<AuthResponse, GatewayError> {
        let url = self.url("/v1/auth/bridge/pat");
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(pat.as_str())
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

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "oauth-client", status, latency_ms)
    )]
    pub async fn provision_oauth_client(
        &self,
        bearer: &BearerToken,
    ) -> Result<BridgeOAuthClientResponse, GatewayError> {
        let url = self.url("/v1/auth/bridge/oauth-client");
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(bearer.expose())
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
            let body = resp.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "hook token rejection body unreadable");
                String::new()
            });
            return Err(GatewayError::HookTokenRejected { status, body });
        }
        resp.json::<HookTokenResponse>()
            .await
            .map_err(|e| GatewayError::HookTokenDecode(Box::new(e)))
    }

    pub(super) async fn post_json<T, R>(
        &self,
        path: &str,
        body: &T,
        endpoint: &'static str,
        session_id: &SessionId,
    ) -> Result<R, GatewayError>
    where
        T: serde::Serialize + Sync,
        R: serde::de::DeserializeOwned,
    {
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
        resp.json::<R>()
            .await
            .map_err(|e| GatewayError::AuthDecode(Box::new(e)))
    }
}
