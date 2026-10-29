//! The administrator trust channel: the `manifestTrust` record in the brand's
//! managed policy store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{GatewayIdentity, PinSource, TrustError, TrustRecord};
use crate::config::store;
use crate::ids::PinnedPubKey;

pub(super) fn policy_trust(gateway: &ValidatedUrl) -> Result<Option<TrustRecord>, TrustError> {
    let Some(raw) = store::read_bridge_policy(store::MANIFEST_TRUST_KEY)? else {
        return Ok(None);
    };
    select_pin(parse_policy_trust(&raw)?, gateway)
}

pub fn select_pin(
    pins: Vec<TrustRecord>,
    gateway: &ValidatedUrl,
) -> Result<Option<TrustRecord>, TrustError> {
    let current = GatewayIdentity::new(gateway)?;
    let mut first = None;
    for pin in pins {
        if pin.gateway == current {
            return Ok(Some(pin));
        }
        first.get_or_insert(pin);
    }
    Ok(first)
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum PolicyPins {
    Set(Vec<TrustRecord>),
    One(TrustRecord),
}

// Why: a machine profile serves every gateway profile on the install, so it
// carries one pin per gateway; a profile installed before that carries a
// single record, which is read as a set of one.
pub fn parse_policy_trust(raw: &str) -> Result<Vec<TrustRecord>, TrustError> {
    let records = match serde_json::from_str(raw).map_err(TrustError::InvalidPolicy)? {
        PolicyPins::Set(records) => records,
        PolicyPins::One(record) => vec![record],
    };
    records
        .into_iter()
        .map(|record| {
            let gateway = ValidatedUrl::try_new(record.gateway.as_str())
                .map_err(TrustError::GatewayInvalid)?;
            // Why: the key is validated by the caller only after the gateway
            // comparison, so a policy pinned for another gateway reports
            // stale rather than a key-decoding error the operator cannot act on.
            Ok(TrustRecord {
                gateway: GatewayIdentity::new(&gateway)?,
                key: PinnedPubKey::new(record.key.as_str()),
                source: PinSource::Policy,
            })
        })
        .collect()
}
