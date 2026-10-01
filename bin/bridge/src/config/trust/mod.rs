//! Gateway-bound signing trust.
//!
//! Trust is a [`TrustRecord`]: the normalized gateway identity, a validated
//! Ed25519 key and where it came from. A managed policy supplies it as
//! `manifestTrust` in the brand's policy store, which only an administrator can
//! write and which no user-controlled input can outrank; an operator pin lives
//! under `[sync.trust]` in the config file. A key that is
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

pub use policy::parse_policy_trust;
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
    InvalidPolicy(#[source] serde_json::Error),
    #[error("signing trust gateway is not a valid URL: {0}")]
    GatewayInvalid(#[source] systemprompt_identifiers::error::IdValidationError),
    #[error("signing trust record cannot be encoded: {0}")]
    RecordEncode(#[source] serde_json::Error),
}

pub fn pinned_pubkey_state() -> Result<PinnedPubkeyState, TrustError> {
    let cfg = Config::load()?;
    pinned_pubkey_state_for(&cfg, &gateway_url_or_default(&cfg))
}

pub fn pinned_pubkey_state_for(
    cfg: &Config,
    gateway: &ValidatedUrl,
) -> Result<PinnedPubkeyState, TrustError> {
    let policy = policy_trust()?;
    let operator = cfg.sync.as_ref().and_then(|s| s.trust.as_ref());
    resolve_pinned_pubkey_state(policy.as_ref(), operator, gateway)
}

pub fn resolve_pinned_pubkey_state(
    policy: Option<&TrustRecord>,
    operator: Option<&TrustRecord>,
    gateway: &ValidatedUrl,
) -> Result<PinnedPubkeyState, TrustError> {
    let current = GatewayIdentity::new(gateway)?;
    let Some(record) = policy.or(operator) else {
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

pub fn persist_pinned_pubkey(gateway: &ValidatedUrl, pubkey: &str) -> Result<(), TrustError> {
    let record = TrustRecord::new(gateway, pubkey, PinSource::Operator)?;
    write::edit(|doc| {
        let configured = write::get(doc, &["gateway_url"]).map_or_else(
            || crate::brand::brand().default_gateway_url,
            |item| item.as_str().unwrap_or(""),
        );
        let configured = GatewayIdentity::parse(configured).map_err(|source| {
            ConfigWriteError::GatewayUnparseable {
                configured: configured.to_owned(),
                source: Box::new(source),
            }
        })?;
        if configured != record.gateway {
            return Err(ConfigWriteError::GatewayChanged {
                expected: record.gateway.to_string(),
                configured: configured.to_string(),
            });
        }
        write::set(doc, &["sync", "trust", "gateway"], record.gateway.as_str())?;
        write::set(doc, &["sync", "trust", "key"], record.key.as_str())?;
        write::set(doc, &["sync", "trust", "source"], "operator")
    })?;
    Ok(())
}
