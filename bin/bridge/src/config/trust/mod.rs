//! Gateway-bound signing trust, including explicit migration of legacy pins.
//!
//! Trust is a [`TrustRecord`]: the normalized gateway identity, a validated
//! Ed25519 key and where it came from. A managed policy supplies it as
//! `manifestTrust`; an operator pin lives under `[sync.trust]` in the config
//! file. A legacy `[sync] pinned_pubkey` written by a pre-0.48 bridge was
//! trust-on-first-use for the gateway configured at the time, so it is adopted
//! as operator trust for the configured gateway (or refused when it names
//! another) and rewritten as `[sync.trust]` by the first sync that verifies
//! against it.
//!
//! A bare policy `manifestPubkey` with no gateway binding is the one legacy
//! form that is adopted rather than refused: older bridges wrote it
//! themselves, and trust resolves at the top of a sync, before the host-sync
//! step that would rewrite it as a bound record, so refusing it stranded those
//! installs with an error no administrator could act on. It is bound to the
//! configured gateway only when nothing else pins at all, which is strictly
//! narrower than the trust it carried when written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ValidatedUrl;

use super::{Config, ConfigReadError, ConfigWriteError, gateway_url_or_default, store, write};
use crate::ids::PinnedPubKey;

mod legacy;
mod policy;

pub use legacy::LegacyPin;
use legacy::adopt_legacy_pin;
use policy::{legacy_unbound_pubkey, policy_trust};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GatewayIdentity(String);

impl TryFrom<String> for GatewayIdentity {
    type Error = TrustError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut url = url::Url::parse(&value)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(TrustError::GatewayShape);
        }
        let path = url.path().trim_end_matches('/').to_owned();
        url.set_path(&path);
        Ok(Self(url.to_string().trim_end_matches('/').to_owned()))
    }
}

impl From<GatewayIdentity> for String {
    fn from(value: GatewayIdentity) -> Self {
        value.0
    }
}

impl GatewayIdentity {
    pub fn new(url: &ValidatedUrl) -> Result<Self, TrustError> {
        Self::try_from(url.as_str().to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSource {
    Policy,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRecord {
    pub gateway: GatewayIdentity,
    pub key: PinnedPubKey,
    pub source: PinSource,
}

impl TrustRecord {
    pub fn new(gateway: &ValidatedUrl, key: &str, source: PinSource) -> Result<Self, TrustError> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(key)?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| TrustError::KeyLength {
                actual: bytes.len(),
            })?;
        ed25519_dalek::VerifyingKey::from_bytes(&bytes)?;
        Ok(Self {
            gateway: GatewayIdentity::new(gateway)?,
            key: PinnedPubKey::new(key),
            source,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyncConfig {
    pub trust: Option<TrustRecord>,
    pub legacy: Option<LegacyPin>,
}

impl SyncConfig {
    #[must_use]
    pub const fn needs_legacy_migration(&self) -> bool {
        self.trust.is_none() && self.legacy.is_some()
    }
}

impl<'de> Deserialize<'de> for SyncConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredSync {
            trust: Option<TrustRecord>,
            pinned_pubkey: Option<String>,
            pinned_pubkey_gateway: Option<String>,
        }
        let stored = StoredSync::deserialize(deserializer)?;
        let legacy = stored
            .pinned_pubkey
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .map(|key| LegacyPin {
                key: PinnedPubKey::new(key),
                gateway: stored.pinned_pubkey_gateway,
            });
        Ok(Self {
            trust: stored.trust,
            legacy,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedPubkeyState {
    Pinned {
        key: PinnedPubKey,
        source: PinSource,
    },
    StaleForGateway {
        pinned_for: String,
        current: String,
    },
    Unpinned,
}

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
    let sync = cfg.sync.as_ref();
    if policy.is_none()
        && let Some(legacy) = sync
            .filter(|s| s.needs_legacy_migration())
            .and_then(|s| s.legacy.as_ref())
    {
        return adopt_legacy_pin(legacy, gateway, &current);
    }
    let Some(record) = policy
        .as_ref()
        .or_else(|| sync.and_then(|s| s.trust.as_ref()))
    else {
        if let Some(key) = legacy_unbound_pubkey()? {
            tracing::warn!(
                target: "bridge::config::trust",
                gateway = %current.0,
                "adopting legacy unbound manifestPubkey for the configured gateway; \
                 the next managed-policy write records it as manifestTrust"
            );
            return Ok(PinnedPubkeyState::Pinned {
                key: TrustRecord::new(gateway, &key, PinSource::Policy)?.key,
                source: PinSource::Policy,
            });
        }
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
                pinned_for: record.gateway.0.clone(),
                current: current.0,
            });
        }
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
        let configured = GatewayIdentity::try_from(configured.to_owned())
            .map_err(|e| ConfigWriteError::GatewayChanged(e.to_string()))?;
        if configured != record.gateway {
            return Err(ConfigWriteError::GatewayChanged(format!(
                "expected {}, configured {}",
                record.gateway.as_str(),
                configured.as_str()
            )));
        }
        write::remove(doc, &["sync", "pinned_pubkey"])?;
        write::remove(doc, &["sync", "pinned_pubkey_gateway"])?;
        write::set(doc, &["sync", "trust", "gateway"], record.gateway.as_str())?;
        write::set(doc, &["sync", "trust", "key"], record.key.as_str())?;
        write::set(doc, &["sync", "trust", "source"], "operator")
    })?;
    Ok(())
}
