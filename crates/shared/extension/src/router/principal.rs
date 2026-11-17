//! The principal the kernel's gateway verifier attached to a request.
//!
//! The kernel inserts a [`GatewayPrincipal`] as a request extension only on
//! routes admitted under a gateway-route conferral, after its verifier has
//! accepted the bearer. A handler extracts it with
//! `Extension<GatewayPrincipal>`; no function outside a handler's extractor
//! takes one as evidence of verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

/// The gateway principal the kernel's gateway verifier attached to a
/// conferred gateway route's request.
#[derive(Debug, Clone)]
pub struct GatewayPrincipal {
    user_id: UserId,
}

impl GatewayPrincipal {
    #[must_use]
    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn __from_kernel(user_id: UserId) -> Self {
        Self { user_id }
    }
}
