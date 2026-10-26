//! The one rule for a gateway 401: whether the rejected token is old enough
//! that a re-mint is worth one try.
//!
//! The proxy's token cache and the bridge-profile fetch both answer a 401
//! with this rule, so a credential is retried, or refused, the same way
//! everywhere. It sits in `gateway`, below both of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

pub const FRESH_REJECTION_WINDOW: Duration = Duration::from_secs(120);

// Why: a gateway rejecting a token minted moments ago means the credential
// itself is revoked; an older one was more likely voided by a redeploy or key
// rotation, and one re-mint is worth trying.
#[must_use]
pub fn rejection_is_stale(token_age: Duration) -> bool {
    token_age > FRESH_REJECTION_WINDOW
}
