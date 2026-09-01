//! Plugin hook-token minting and the per-context freshness cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{OAuthClientCreds, PluginOAuthError, ensure_creds, refresh_creds};
use crate::gateway::{GatewayClient, GatewayError, HookTokenResponse};
use crate::ids::BearerToken;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use systemprompt_identifiers::PluginId;

pub const REFRESH_THRESHOLD_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct CachedHookToken {
    pub access_token: String,
    pub expires_at_unix: u64,
}

impl CachedHookToken {
    fn from_response(r: HookTokenResponse) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let lifetime = u64::try_from(r.expires_in.max(0)).unwrap_or(0);
        Self {
            access_token: r.access_token,
            expires_at_unix: now.saturating_add(lifetime),
        }
    }

    fn is_fresh(&self, threshold_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.expires_at_unix > now.saturating_add(threshold_secs)
    }
}

const KEY_SEP: char = '\u{1f}';

fn cache_key(gateway: &str, plugin_id: &PluginId) -> String {
    format!("{gateway}{KEY_SEP}{}", plugin_id.as_str())
}

/// Hook tokens held in memory, keyed on the gateway that minted them as well as
/// the plugin they were minted for.
///
/// Why the gateway is part of the key: a hook token is signed by one gateway's
/// RSA authority and is meaningless to any other. Keyed on the plugin alone,
/// repointing the bridge from production to a local server handed the
/// still-fresh production token to the local governance webhook, which rejected
/// it as an unknown signing key and blocked every tool call — with nothing to
/// invalidate the entry, so it never recovered.
#[derive(Debug, Default)]
pub struct PluginTokenCache {
    entries: Mutex<HashMap<String, CachedHookToken>>,
}

impl PluginTokenCache {
    // Why: recovers from a poisoned lock — treating poison as a miss would
    // silently re-mint a hook token on every request from then on.
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedHookToken>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn get(
        &self,
        gateway: &str,
        plugin_id: &PluginId,
        threshold_secs: u64,
    ) -> Option<CachedHookToken> {
        let cached = self.entries().get(&cache_key(gateway, plugin_id))?.clone();
        cached.is_fresh(threshold_secs).then_some(cached)
    }

    pub fn put(&self, gateway: &str, plugin_id: &PluginId, token: CachedHookToken) {
        self.entries().insert(cache_key(gateway, plugin_id), token);
    }

    pub fn invalidate(&self, gateway: &str, plugin_id: &PluginId) {
        self.entries().remove(&cache_key(gateway, plugin_id));
    }

    pub fn invalidate_plugin(&self, plugin_id: &PluginId) {
        let suffix = format!("{KEY_SEP}{}", plugin_id.as_str());
        self.entries().retain(|key, _| !key.ends_with(&suffix));
    }
}

async fn mint(
    gateway: &GatewayClient,
    c: &OAuthClientCreds,
    plugin_id: &PluginId,
) -> Result<HookTokenResponse, GatewayError> {
    let endpoint = gateway_aligned_endpoint(&c.token_endpoint, gateway.base_url_str());
    gateway
        .mint_plugin_hook_token(&endpoint, &c.client_id, &c.client_secret, plugin_id)
        .await
}

fn gateway_aligned_endpoint(raw: &str, gateway: &str) -> String {
    let (Ok(mut parsed), Ok(gw)) = (url::Url::parse(raw), url::Url::parse(gateway)) else {
        return raw.to_owned();
    };
    let is_loopback = match parsed.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(a)) => a.is_loopback(),
        Some(url::Host::Ipv6(a)) => a.is_loopback(),
        None => false,
    };
    if !is_loopback {
        return raw.to_owned();
    }
    let Some(gw_host) = gw.host_str() else {
        return raw.to_owned();
    };
    if parsed.set_scheme(gw.scheme()).is_err()
        || parsed.set_host(Some(gw_host)).is_err()
        || parsed.set_port(gw.port()).is_err()
    {
        return raw.to_owned();
    }
    let rebuilt = parsed.to_string();
    tracing::info!(
        target: "bridge::auth::plugin-oauth",
        original = %raw,
        rewritten = %rebuilt,
        "gateway advertised a loopback hook-token endpoint; re-pointed it at the configured gateway"
    );
    rebuilt
}

pub async fn mint_or_refresh_plugin_token(
    cache: &PluginTokenCache,
    gateway: &GatewayClient,
    bearer: &BearerToken,
    plugin_id: &PluginId,
) -> Result<CachedHookToken, PluginOAuthError> {
    let gateway_key = gateway.base_url_str().to_owned();
    if let Some(cached) = cache.get(&gateway_key, plugin_id, REFRESH_THRESHOLD_SECS) {
        return Ok(cached);
    }
    let creds = ensure_creds(gateway, bearer).await?;
    let response = match mint(gateway, &creds, plugin_id).await {
        Ok(r) => r,
        Err(GatewayError::HookTokenRejected { status, .. }) if status.as_u16() == 401 => {
            tracing::warn!(
                plugin_id = plugin_id.as_str(),
                "hook token mint 401; rotating client secret and retrying"
            );
            let creds = refresh_creds(gateway, bearer).await?;
            mint(gateway, &creds, plugin_id).await?
        },
        Err(e) => return Err(e.into()),
    };
    let cached = CachedHookToken::from_response(response);
    cache.put(&gateway_key, plugin_id, cached.clone());
    Ok(cached)
}
