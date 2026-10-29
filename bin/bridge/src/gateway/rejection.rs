//! The one rule for a gateway 401: a rejected token is re-minted once, and the
//! credential is held revoked only when a token minted moments ago is
//! rejected again.
//!
//! The proxy's token cache and the bridge-profile fetch both answer a 401
//! with this rule, so a credential is retried, or refused, the same way
//! everywhere. It sits in `gateway`, below both of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

pub const FRESH_REJECTION_WINDOW: Duration = Duration::from_secs(120);

// Why: a gateway restart or key rotation voids even a token minted moments
// ago, so a fresh rejection alone never proves revocation. A stale rejection
// always earns a re-mint; a fresh one earns exactly one, and a revoked
// credential then fails the mint itself or is rejected again.
#[must_use]
pub fn rejection_is_stale(token_age: Duration) -> bool {
    token_age > FRESH_REJECTION_WINDOW
}
