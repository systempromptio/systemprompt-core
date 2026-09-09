//! Adoption of the pre-0.48 `[sync] pinned_pubkey` as operator trust.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{GatewayIdentity, PinSource, PinnedPubkeyState, TrustError, TrustRecord};
use crate::ids::PinnedPubKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPin {
    pub key: PinnedPubKey,
    pub gateway: Option<String>,
}

// Why: a pre-0.48 pin was learned by trust-on-first-use for whichever gateway
// was configured, which is exactly the operator trust 0.48 models per gateway.
// Refusing it stranded every upgraded install behind an error whose only
// remedy was an administrator command, so it is adopted for the same gateway
// and, like any operator pin, dropped for a different one.
pub(super) fn adopt_legacy_pin(
    legacy: &LegacyPin,
    gateway: &ValidatedUrl,
    current: &GatewayIdentity,
) -> Result<PinnedPubkeyState, TrustError> {
    if let Some(recorded) = legacy.gateway.as_deref() {
        let recorded = GatewayIdentity::try_from(recorded.to_owned())?;
        if recorded != *current {
            tracing::warn!(
                target: "bridge::config::trust",
                pinned_for = %recorded.0,
                gateway = %current.0,
                "legacy pin names another gateway; trusting the configured gateway on first use",
            );
            return Ok(PinnedPubkeyState::Unpinned);
        }
    }
    let validated = TrustRecord::new(gateway, legacy.key.as_str(), PinSource::Operator)?;
    tracing::info!(
        target: "bridge::config::trust",
        gateway = %current.0,
        "adopting legacy [sync] pinned_pubkey as operator trust; the next verified sync rewrites it as [sync.trust]"
    );
    Ok(PinnedPubkeyState::Pinned {
        key: validated.key,
        source: PinSource::Operator,
    })
}
