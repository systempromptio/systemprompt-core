//! Bridge profile parsing, including the native policy public key section.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use std::env;

use systemprompt_identifiers::ValidatedUrl;

use super::{Config, default_gateway};
use crate::ids::PinnedPubKey;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeConfig {
    #[serde(default)]
    pub inference_gateway_base_url: Option<ValidatedUrl>,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

#[must_use]
pub fn gateway_url_or_default(cfg: &Config) -> ValidatedUrl {
    let url = cfg.gateway_url.clone().unwrap_or_else(default_gateway);
    tracing::debug!(gateway = %url, "gateway resolved");
    url
}

/// Where the effective manifest pubkey came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinSource {
    /// Supplied out of band (env var or OS managed policy). Always
    /// authoritative.
    Policy,
    /// Learned by trust-on-first-use and written to the config file for one
    /// gateway.
    Operator,
}

impl PinSource {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Policy => "policy (env or managed policy)",
            Self::Operator => "config file",
        }
    }
}

/// The pin as it applies to the *current* gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedPubkeyState {
    Pinned {
        key: PinnedPubKey,
        source: PinSource,
    },
    /// The config file holds a key learned for a different gateway. It is not
    /// used; the next sync re-learns the key for the current gateway.
    StaleForGateway {
        pinned_for: String,
        current: String,
    },
    Unpinned,
}

/// The scheme, host and port a pin is bound to. Path, query and trailing
/// slashes are irrelevant to which signer is behind the URL.
#[must_use]
pub fn gateway_origin(url: &ValidatedUrl) -> String {
    url::Url::parse(url.as_str()).map_or_else(
        |_| url.as_str().trim_end_matches('/').to_ascii_lowercase(),
        |u| u.origin().ascii_serialization(),
    )
}

#[must_use]
pub fn pinned_pubkey_state() -> PinnedPubkeyState {
    if let Some(key) = policy_pubkey() {
        return PinnedPubkeyState::Pinned {
            key,
            source: PinSource::Policy,
        };
    }
    let cfg = Config::load();
    let Some(sync) = cfg.sync.as_ref() else {
        return PinnedPubkeyState::Unpinned;
    };
    let Some(key) = sync.pinned_pubkey.clone() else {
        return PinnedPubkeyState::Unpinned;
    };
    let current = gateway_origin(&gateway_url_or_default(&cfg));
    // Why: a pin without a gateway predates gateway-keyed pins. Treating it as
    // stale (not as valid for whatever gateway is configured now) is what stops
    // a key learned from one gateway silently failing every sync against another.
    let pinned_for = sync.pinned_pubkey_gateway.clone().unwrap_or_default();
    if pinned_for == current {
        PinnedPubkeyState::Pinned {
            key,
            source: PinSource::Operator,
        }
    } else {
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        }
    }
}

#[must_use]
pub fn pinned_pubkey() -> Option<PinnedPubKey> {
    match pinned_pubkey_state() {
        PinnedPubkeyState::Pinned { key, .. } => Some(key),
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            tracing::info!(
                pinned_for = %pinned_for,
                current = %current,
                "config-file manifest pubkey was pinned for another gateway; ignoring it"
            );
            None
        },
        PinnedPubkeyState::Unpinned => None,
    }
}

#[must_use]
pub fn policy_pubkey() -> Option<PinnedPubKey> {
    if let Ok(value) = env::var(crate::brand::brand().env("POLICY_PUBKEY")) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PinnedPubKey::new(trimmed));
        }
    }
    read_policy_pubkey_native().map(PinnedPubKey::new)
}

fn read_policy_pubkey_native() -> Option<String> {
    let value = super::store::read_bridge_policy(super::store::MANIFEST_PUBKEY_KEY)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Pins `pubkey` for the gateway currently configured. The gateway origin is
/// stored beside the key so a later gateway change invalidates the pin
/// instead of failing every sync against the new signer.
pub fn persist_pinned_pubkey(pubkey: &str) -> Result<(), super::ConfigWriteError> {
    let origin = gateway_origin(&gateway_url_or_default(&Config::load()));
    super::write::edit(|doc| {
        super::write::set(doc, &["sync", "pinned_pubkey"], pubkey);
        super::write::set(doc, &["sync", "pinned_pubkey_gateway"], origin.as_str());
    })
}
