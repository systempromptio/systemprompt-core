//! Governance hooks: the per-user hook client, hook tokens, and the track and
//! govern calls.
//!
//! The hook client is provisioned with the user's JWT; its token endpoint is
//! used as advertised only on the instance's own origin, otherwise its path on
//! that origin. A hook token is minted per plugin with `client_credentials`.
//! Track (5 s) and govern (3 s) take the hook token, never the JWT; neither
//! retries, and the caller decides what a failure means (track drops the event,
//! govern lets the tool run). The device credential goes to track only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use crate::protocol::origin::{align_token_endpoint, bearer_allowed};
use crate::protocol::wire::{HookTokenResponse, OAuthClientResponse};
use crate::protocol::{ClientSecret, DeviceCredential, HookToken, Jwt, OriginError, paths};
use chrono::{DateTime, TimeDelta, Utc};
use reqwest::header::CONTENT_TYPE;
use systemprompt_identifiers::{ClientId, PluginId, headers};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::hook_events::{
    GovernAnswer, HOOK_TIMEOUT_MS, HOOK_TOKEN_REFRESH_SECS, HookEvent, VERDICT_TIMEOUT_MS,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::session::HookTokenRequest;
use url::Url;
use uuid::Uuid;

use crate::client::error::ClientError;
use crate::client::http::{SMALL_BODY, decode, exchange, failure, protocol};
use crate::client::instance::SpClient;

const OAUTH_CLIENT_ENDPOINT: &str = "POST /v1/auth/bridge/oauth-client";
const HOOK_TOKEN_ENDPOINT: &str = "POST {token endpoint}";
const TRACK_ENDPOINT: &str = "POST /api/public/hooks/track";
const GOVERN_ENDPOINT: &str = "POST /api/public/hooks/govern";

const HOOK_TIMEOUT: Duration = Duration::from_millis(HOOK_TIMEOUT_MS);
const VERDICT_TIMEOUT: Duration = Duration::from_millis(VERDICT_TIMEOUT_MS);

/// The user's hook OAuth client, held in memory by the caller. Provisioning
/// again rotates its secret.
#[derive(Debug, Clone)]
pub struct HookClient {
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
    pub token_endpoint: Url,
    pub scopes: Vec<String>,
}

/// A hook token for one plugin and when it lapses.
#[derive(Debug, Clone)]
pub struct HookGrant {
    pub token: HookToken,
    pub expires_in: Duration,
    pub expires_at: DateTime<Utc>,
}

impl HookGrant {
    #[must_use]
    pub fn needs_refresh(&self, now: DateTime<Utc>) -> bool {
        TimeDelta::try_seconds(HOOK_TOKEN_REFRESH_SECS)
            .and_then(|margin| now.checked_add_signed(margin))
            .is_none_or(|threshold| threshold >= self.expires_at)
    }
}

impl SpClient {
    pub async fn provision_hook_client(&self, jwt: &Jwt) -> Result<HookClient, ClientError> {
        let request = self
            .post(self.at(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT)?)
            .bearer_auth(jwt.expose())
            .header(CONTENT_TYPE, "application/json")
            .body("{}");
        let answer = exchange(OAUTH_CLIENT_ENDPOINT, request, HOOK_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(OAUTH_CLIENT_ENDPOINT, &answer));
        }
        let response: OAuthClientResponse = decode(OAUTH_CLIENT_ENDPOINT, &answer)?;
        let aligned = align_token_endpoint(response.token_endpoint.as_deref(), self.origin());
        let token_endpoint = Url::parse(&aligned)
            .ok()
            .filter(|endpoint| bearer_allowed(endpoint, self.base_url()))
            .ok_or_else(|| {
                protocol(
                    OAUTH_CLIENT_ENDPOINT,
                    Some(200),
                    "the hook token endpoint leaves the instance",
                )
            })?;
        Ok(HookClient {
            client_id: response.client_id,
            client_secret: response.client_secret,
            token_endpoint,
            scopes: response.scopes,
        })
    }

    pub async fn mint_hook_token(
        &self,
        client: &HookClient,
        plugin_id: &PluginId,
    ) -> Result<HookGrant, ClientError> {
        if !bearer_allowed(&client.token_endpoint, self.base_url()) {
            return Err(ClientError::Origin(OriginError::Foreign));
        }
        let form =
            HookTokenRequest::new(&client.client_id, client.client_secret.expose(), plugin_id);
        let request = self.post(client.token_endpoint.clone()).form(&form);
        let answer = exchange(HOOK_TOKEN_ENDPOINT, request, HOOK_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(HOOK_TOKEN_ENDPOINT, &answer));
        }
        let response: HookTokenResponse = decode(HOOK_TOKEN_ENDPOINT, &answer)?;
        let lifetime = response.expires_in.max(0);
        let expires_in = Duration::from_secs(u64::try_from(lifetime).unwrap_or(0));
        let now = Utc::now();
        let expires_at = TimeDelta::try_seconds(lifetime)
            .and_then(|delta| now.checked_add_signed(delta))
            .unwrap_or(now);
        Ok(HookGrant {
            token: response.access_token,
            expires_in,
            expires_at,
        })
    }

    pub async fn track(
        &self,
        token: &HookToken,
        plugin_id: &PluginId,
        event: &HookEvent,
        device: Option<&DeviceCredential>,
    ) -> Result<(), ClientError> {
        let ingestion = match event.ingestion_id() {
            id if !id.is_empty() && id.bytes().all(|b| b.is_ascii_graphic()) => id.to_owned(),
            _ => Uuid::new_v4().hyphenated().to_string(),
        };
        let mut request = self
            .post(self.hook_url(ApiPaths::PUBLIC_HOOKS_TRACK, plugin_id)?)
            .bearer_auth(token.expose())
            .header(headers::HOST, HostKind::Harness.as_str())
            .header(headers::INGESTION_EVENT_ID, ingestion)
            .json(event);
        if let Some(device) = device {
            request = request.header(headers::DEVICE_CREDENTIAL, device.expose());
        }
        let answer = exchange(TRACK_ENDPOINT, request, HOOK_TIMEOUT, SMALL_BODY).await?;
        if !answer.status.is_success() {
            return Err(failure(TRACK_ENDPOINT, &answer));
        }
        Ok(())
    }

    pub async fn govern(
        &self,
        token: &HookToken,
        plugin_id: &PluginId,
        event: &HookEvent,
    ) -> Result<GovernAnswer, ClientError> {
        let request = self
            .post(self.hook_url(paths::HOOKS_GOVERN, plugin_id)?)
            .bearer_auth(token.expose())
            .header(headers::HOST, HostKind::Harness.as_str())
            .json(event);
        let answer = exchange(GOVERN_ENDPOINT, request, VERDICT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(GOVERN_ENDPOINT, &answer));
        }
        decode(GOVERN_ENDPOINT, &answer)
    }

    fn hook_url(&self, path: &str, plugin_id: &PluginId) -> Result<Url, ClientError> {
        let mut url = self.at(path)?;
        url.query_pairs_mut()
            .append_pair("plugin_id", plugin_id.as_str());
        Ok(url)
    }
}
