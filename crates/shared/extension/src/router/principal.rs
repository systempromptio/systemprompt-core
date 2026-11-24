//! The principal the kernel's gateway verifier attached to a request.
//!
//! The kernel inserts a [`GatewayPrincipal`] as a request extension only on
//! routes admitted under a gateway-route conferral, after its verifier has
//! accepted the bearer. A handler extracts it with
//! `Extension<GatewayPrincipal>` and hands it to the caller-bound views
//! conferred on `gateway-http`, which read and write only that principal's
//! rows. Nothing outside the kernel's verifier builds one: the one
//! constructor is hidden, and `lint-hidden-constructors` refuses a call to it
//! anywhere in core but the API's bridge credential check. Every API that
//! takes a principal is a sealed or conferred view no install resolves, so an
//! install that forges one reaches nothing with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;

/// The catalog credential names a gateway principal stored a personal key for.
///
/// Nothing else: no key material, no write, no other
/// principal. The `ai` member provides it sealed; the kernel's bridge
/// profile reads it. Held as `Arc<dyn PersonalCredentialNames>`, hence
/// `#[async_trait]`.
#[async_trait::async_trait]
pub trait PersonalCredentialNames: Send + Sync {
    async fn credential_names(
        &self,
        caller: &GatewayPrincipal,
    ) -> Result<BTreeSet<String>, RepositoryError>;
}

pub type DynPersonalCredentialNames = Arc<dyn PersonalCredentialNames>;

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
