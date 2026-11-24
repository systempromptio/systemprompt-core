//! [`IdentityHttpExtension`] (`identity-http`): the identity HTTP surface.
//!
//! With `http` it contributes the open
//! [`DynIdentitySurface`](systemprompt_oauth_issuance::surface::DynIdentitySurface)
//! the API mount builds the OAuth, SSO, self-service session and admin
//! API-key routes from (see [`crate::http`]), and provides sealed the
//! [`IdentityHttpRepositories`](crate::http::IdentityHttpRepositories) those
//! routes read. The surface reads the sealed kernel-seeded
//! `Arc<OAuthRepositories>`, conferred on `identity-http` under the
//! `identity-oauth-repositories` row, when the API builds it; a child host
//! seeds none and never builds the surface, so the member declares no
//! boot-time requirement on it. The API
//! keeps their layering: the policies, context kinds, rate scopes and budgets,
//! the file-managed client-write refusal and the route groups.
//!
//! The member is its own capability unit (facade feature `identity`), not part
//! of the kernel: a profile that disables `identity-http` serves no OAuth, SSO,
//! `/api/v1/core/users` or `/api/v1/admin/api-keys` route (404) and no OAuth
//! discovery document, while the kernel keeps authenticating requests through
//! the users member's request authenticator and minting the anonymous session
//! cookie.
//!
//! | profile disables | identity routes |
//! |---|---|
//! | `identity-http` | 404 (not mounted) |
//! | `users` | boot refused (`users` is a required member) |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const IDENTITY_HTTP_EXTENSION: &str = "identity-http";

/// The member that contributes the identity HTTP surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityHttpExtension;

impl Extension for IdentityHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: IDENTITY_HTTP_EXTENSION,
            name: "Identity HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(feature = "http"),
        expect(
            unused_variables,
            reason = "the member contributes only its surface, which the http feature compiles"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        surface::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod surface {
    use std::sync::Arc;

    use systemprompt_database::DbPool;
    use systemprompt_extension::prelude::*;
    use systemprompt_oauth_issuance::surface::DynIdentitySurface;
    use systemprompt_users::UserRepository;

    use crate::http::{IdentityHttpRepositories, IdentityHttpSurface};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.provide(
            |_r: &Resolved<'_>| -> Result<DynIdentitySurface, ProvideError> {
                Ok(Arc::new(IdentityHttpSurface))
            },
        )
        .provide_sealed(repositories);
    }

    fn repositories(r: &Resolved<'_>) -> Result<Arc<IdentityHttpRepositories>, ProvideError> {
        let db = r.get::<DbPool>()?;
        Ok(Arc::new(IdentityHttpRepositories {
            users: Arc::new(UserRepository::new(&db)),
        }))
    }
}
