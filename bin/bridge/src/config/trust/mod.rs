//! Gateway-bound signing trust.
//!
//! Trust is a [`TrustRecord`]: the normalized gateway identity, a validated
//! Ed25519 key and where it came from. A managed policy supplies it as
//! `manifestTrust` (or the `<PREFIX>_POLICY_TRUST` environment override); an
//! operator pin lives under `[sync.trust]` in the config file. A key that is
//! not bound to a gateway never pins: a record for another gateway is stale
//! when it came from policy and simply not in effect when it is an operator
//! pin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{Config, ConfigReadError, ConfigWriteError, gateway_url_or_default, store, write};
use crate::ids::PinnedPubKey;

mod policy;
mod record;

use policy::policy_trust;
pub use record::{GatewayIdentity, PinSource, PinnedPubkeyState, SyncConfig, TrustRecord};

#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error(transparent)]
    Config(#[from] ConfigReadError),
    #[error(transparent)]
    Store(#[from] store::ConfigStoreError),
    #[error(transparent)]
    Write(#[from] ConfigWriteError),
    #[error("signing trust gateway is not a URL: {0}")]
    GatewayUnparseable(#[from] url::ParseError),
    #[error(
        "signing trust requires an HTTP(S) gateway base URL without credentials, query or fragment"
    )]
    GatewayShape,
    #[error("signing trust key is not base64: {0}")]
    KeyEncoding(#[from] base64::DecodeError),
    #[error("signing trust key decodes to {actual} bytes; an Ed25519 public key is 32")]
    KeyLength { actual: usize },
    #[error("signing trust key is not a valid Ed25519 public key: {0}")]
    KeyInvalid(#[from] ed25519_dalek::SignatureError),
    #[error(
        "managed signing trust is invalid: {0}; configure manifestTrust with gateway, key and source"
    )]
    InvalidPolicy(String),
}

pub fn pinned_pubkey_state() -> Result<PinnedPubkeyState, TrustError> {
    let cfg = Config::load()?;
    pinned_pubkey_state_for(&cfg, &gateway_url_or_default(&cfg))
}

pub fn pinned_pubkey_state_for(
    cfg: &Config,
    gateway: &ValidatedUrl,
) -> Result<PinnedPubkeyState, TrustError> {
    let current = GatewayIdentity::new(gateway)?;
    let policy = policy_trust()?;
    let operator = cfg.sync.as_ref().and_then(|s| s.trust.as_ref());
    let Some(record) = policy.as_ref().or(operator) else {
        return Ok(PinnedPubkeyState::Unpinned);
    };
    // Why: a record for another gateway is stale whatever its key looks
    // like; validating the key first turned "pinned for a different
    // gateway" into a decoding error the operator could not act on.
    if record.gateway != current {
        // Why: a managed pin names a key for one gateway, so pointing at another
        // is a conflict only the administrator can resolve. An operator pin is
        // trust learned by first use, which is per gateway by nature: a second
        // gateway is a new trust domain, and reporting it stale left every
        // legitimate gateway switch with no remedy short of an admin command.
        if policy.is_some() {
            return Ok(PinnedPubkeyState::StaleForGateway {
                pinned_for: record.gateway.clone(),
                current,
            });
        }
        tracing::warn!(
            target: "bridge::config::trust",
            pinned_for = %record.gateway,
            current = %current,
            "operator pin names another gateway; it is not in effect for the configured gateway"
        );
        return Ok(PinnedPubkeyState::Unpinned);
    }
    let validated = TrustRecord::new(gateway, record.key.as_str(), record.source)?;
    Ok(PinnedPubkeyState::Pinned {
        key: validated.key,
        source: if policy.is_some() {
            PinSource::Policy
        } else {
            PinSource::Operator
        },
    })
}

pub fn pinned_pubkey() -> Result<Option<PinnedPubKey>, TrustError> {
    Ok(match pinned_pubkey_state()? {
        PinnedPubkeyState::Pinned { key, .. } => Some(key),
        PinnedPubkeyState::Unpinned | PinnedPubkeyState::StaleForGateway { .. } => None,
    })
}

pub fn policy_pubkey() -> Result<Option<PinnedPubKey>, TrustError> {
    policy_trust()?
        .map(|record| {
            let gateway = ValidatedUrl::try_new(record.gateway.as_str())
                .map_err(|e| TrustError::InvalidPolicy(format!("gateway: {e}")))?;
            Ok(TrustRecord::new(&gateway, record.key.as_str(), PinSource::Policy)?.key)
        })
        .transpose()
}

pub fn persist_pinned_pubkey(gateway: &ValidatedUrl, pubkey: &str) -> Result<(), TrustError> {
    let record = TrustRecord::new(gateway, pubkey, PinSource::Operator)?;
    write::edit(|doc| {
        let configured = write::get(doc, &["gateway_url"]).map_or_else(
            || crate::brand::brand().default_gateway_url,
            |item| item.as_str().unwrap_or(""),
        );
        let configured = GatewayIdentity::parse(configured)
            .map_err(|e| ConfigWriteError::GatewayChanged(e.to_string()))?;
        if configured != record.gateway {
            return Err(ConfigWriteError::GatewayChanged(format!(
                "expected {}, configured {}",
                record.gateway, configured
            )));
        }
        write::set(doc, &["sync", "trust", "gateway"], record.gateway.as_str())?;
        write::set(doc, &["sync", "trust", "key"], record.key.as_str())?;
        write::set(doc, &["sync", "trust", "source"], "operator")
    })?;
    Ok(())
}
