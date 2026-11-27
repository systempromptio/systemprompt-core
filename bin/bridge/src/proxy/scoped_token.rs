//! Scoped credentials derived from the loopback secret.
//!
//! The loopback secret itself lives only in bridge-owned 0600 files. Every
//! surface another local account can read — a plugin's `hooks.json` under
//! org-plugins, the Claude Desktop managed-preferences plist or policy hive —
//! carries a token derived from it instead: HMAC-SHA256 keyed with the secret
//! over a scope label. A leaked hook token drives only that plugin's hook
//! endpoints; a leaked host token drives only inference and managed MCP for
//! that host. Neither reconstructs the secret, and a secret reset invalidates
//! every derived token at once.
//!
//! A host token exists only for a host whose configuration the bridge owns
//! ([`ConfigOwner::Bridge`]): [`host_token`] refuses any other host and
//! [`verify`] accepts no token under its scope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::digest::KeyInit as _;
use hmac::{Hmac, Mac as _};
use sha2::{Digest as _, Sha256};

use systemprompt_models::bridge::host::{ConfigOwner, HostKind};

use crate::ids::{HookToken, HostToken, LoopbackSecret, PluginId, ProxySecret};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenScope {
    Hook(PluginId),
    Host(HostKind),
}

impl TokenScope {
    fn label(&self) -> String {
        match self {
            Self::Hook(plugin) => format!("hook:{}", plugin.as_str()),
            Self::Host(host) => format!("host:{}", host.as_str()),
        }
    }
}

// Why: `Hmac::new` takes a block-sized key; hashing the secret first gives a
// fixed 32-byte key without a fallible length check.
fn derive_raw(secret: &[u8], scope: &TokenScope) -> String {
    let mut key = hmac::digest::Key::<Hmac<Sha256>>::default();
    key[..32].copy_from_slice(&Sha256::digest(secret));
    let mut mac = Hmac::<Sha256>::new(&key);
    mac.update(scope.label().as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

#[must_use]
pub fn hook_token(secret: &LoopbackSecret, plugin: &PluginId) -> HookToken {
    HookToken::new(derive_raw(
        secret.as_str().as_bytes(),
        &TokenScope::Hook(plugin.clone()),
    ))
}

/// The refusal to issue or accept a loopback credential for a host that owns
/// its configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("host `{0}` configures itself; the bridge issues it no loopback credential")]
pub struct HostOwnsConfig(pub HostKind);

fn derive_host(secret: &LoopbackSecret, host: HostKind) -> HostToken {
    HostToken::new(derive_raw(
        secret.as_str().as_bytes(),
        &TokenScope::Host(host),
    ))
}

pub fn host_token(secret: &LoopbackSecret, host: HostKind) -> Result<HostToken, HostOwnsConfig> {
    match host.config_owner() {
        ConfigOwner::Bridge => Ok(derive_host(secret, host)),
        ConfigOwner::Host => Err(HostOwnsConfig(host)),
    }
}

#[must_use]
pub fn desktop_host_token(secret: &LoopbackSecret) -> HostToken {
    derive_host(secret, HostKind::ClaudeDesktop)
}

#[must_use]
pub fn desktop_policy_fingerprint(secret: &LoopbackSecret) -> String {
    super::secret::fingerprint(desktop_host_token(secret).as_str())
}

#[must_use]
pub fn verify(presented: &str, secret: &ProxySecret, scope: &TokenScope) -> bool {
    if matches!(scope, TokenScope::Host(host) if host.config_owner() != ConfigOwner::Bridge) {
        return false;
    }
    let expected = derive_raw(secret.as_str().as_bytes(), scope);
    super::secret::verify(presented, &ProxySecret::new(expected))
}
