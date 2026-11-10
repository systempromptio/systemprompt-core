//! First trust of a gateway's manifest-signing key from the GUI.
//!
//! The GUI never pins a key on first use by itself. It shows the user the
//! key's fingerprint, and a pin is written only when the user confirms that
//! fingerprint for that gateway and the key the gateway serves at confirm
//! time still has it. The fingerprint is the SHA-256 of the key exactly as
//! the gateway's pubkey endpoint serves it, so an operator can reproduce it
//! from the same response.

use systemprompt_identifiers::ValidatedUrl;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TrustConfirmError {
    #[error("the active gateway is {active}, not the {confirmed} the key was confirmed for")]
    GatewayChanged { confirmed: String, active: String },
    #[error(
        "the gateway now serves a key with fingerprint {served}, not the {confirmed} you confirmed"
    )]
    FingerprintMismatch { confirmed: String, served: String },
}

#[must_use]
pub fn key_fingerprint(pubkey: &str) -> String {
    crate::hash::sha256_hex(pubkey.trim().as_bytes())
}

fn normalise(fingerprint: &str) -> String {
    fingerprint
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn confirm<'k>(
    confirmed_gateway: &str,
    confirmed_fingerprint: &str,
    active_gateway: &ValidatedUrl,
    served_key: &'k str,
) -> Result<&'k str, TrustConfirmError> {
    if confirmed_gateway != active_gateway.as_str() {
        return Err(TrustConfirmError::GatewayChanged {
            confirmed: confirmed_gateway.to_owned(),
            active: active_gateway.as_str().to_owned(),
        });
    }
    let served = key_fingerprint(served_key);
    let confirmed = normalise(confirmed_fingerprint);
    if confirmed != served {
        return Err(TrustConfirmError::FingerprintMismatch {
            confirmed: confirmed_fingerprint.to_owned(),
            served,
        });
    }
    Ok(served_key.trim())
}
