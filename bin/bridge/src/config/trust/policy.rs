//! The administrator trust channel: `manifestTrust` (or the environment
//! override) and the bare legacy `manifestPubkey`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{GatewayIdentity, PinSource, TrustError, TrustRecord};
use crate::config::store;
use crate::ids::PinnedPubKey;

pub(super) fn policy_trust() -> Result<Option<TrustRecord>, TrustError> {
    let env_name = crate::brand::brand().env("POLICY_TRUST");
    let raw = match std::env::var(&env_name) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => {
            store::read_bridge_policy(store::MANIFEST_TRUST_KEY)?
        },
        Err(e) => return Err(TrustError::InvalidPolicy(format!("{env_name}: {e}"))),
    };
    if let Some(raw) = raw {
        let record: TrustRecord =
            serde_json::from_str(&raw).map_err(|e| TrustError::InvalidPolicy(e.to_string()))?;
        let gateway = ValidatedUrl::try_new(record.gateway.as_str())
            .map_err(|e| TrustError::InvalidPolicy(format!("gateway: {e}")))?;
        // Why: the key is validated by the caller only after the gateway
        // comparison, so a policy pinned for another gateway reports stale
        // rather than a key-decoding error the operator cannot act on.
        return Ok(Some(TrustRecord {
            gateway: GatewayIdentity::new(&gateway)?,
            key: PinnedPubKey::new(record.key.as_str()),
            source: PinSource::Policy,
        }));
    }
    Ok(None)
}

pub(super) fn legacy_unbound_pubkey() -> Result<Option<String>, TrustError> {
    let pubkey_env = crate::brand::brand().env("POLICY_PUBKEY");
    let raw = match std::env::var(&pubkey_env) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => {
            store::read_bridge_policy(store::MANIFEST_PUBKEY_KEY)?
        },
        Err(e) => return Err(TrustError::InvalidPolicy(format!("{pubkey_env}: {e}"))),
    };
    Ok(raw
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty()))
}
