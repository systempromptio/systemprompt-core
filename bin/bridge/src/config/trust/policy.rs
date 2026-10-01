//! The administrator trust channel: the `manifestTrust` record in the brand's
//! managed policy store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{GatewayIdentity, PinSource, TrustError, TrustRecord};
use crate::config::store;
use crate::ids::PinnedPubKey;

pub(super) fn policy_trust() -> Result<Option<TrustRecord>, TrustError> {
    store::read_bridge_policy(store::MANIFEST_TRUST_KEY)?
        .map(|raw| parse_policy_trust(&raw))
        .transpose()
}

pub fn parse_policy_trust(raw: &str) -> Result<TrustRecord, TrustError> {
    let record: TrustRecord =
        serde_json::from_str(raw).map_err(|e| TrustError::InvalidPolicy(e.to_string()))?;
    let gateway = ValidatedUrl::try_new(record.gateway.as_str())
        .map_err(|e| TrustError::InvalidPolicy(format!("gateway: {e}")))?;
    // Why: the key is validated by the caller only after the gateway
    // comparison, so a policy pinned for another gateway reports stale
    // rather than a key-decoding error the operator cannot act on.
    Ok(TrustRecord {
        gateway: GatewayIdentity::new(&gateway)?,
        key: PinnedPubKey::new(record.key.as_str()),
        source: PinSource::Policy,
    })
}
