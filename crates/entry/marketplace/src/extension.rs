//! [`MarketplaceHttpExtension`] (`marketplace-http`): the marketplace route
//! set.
//!
//! With `http` it contributes one route set at `/api/v1/marketplace` in the
//! content-and-misc role group, behind the public gate with a public request
//! context, under the `marketplace` rate-limit scope with the content budget
//! and the default body limit. Its shape equals the reference composition's
//! conferral for `marketplace-http` at that base. The handlers read the
//! managed repository and the catalogue cache the `managed_resources` member
//! provides, which this member `requires`.
//!
//! The `core marketplace`, `core plugins` and `core skills` command paths
//! are conferred to this member; the commands themselves are still served by
//! the CLI kernel.
//!
//! | profile disables | `/api/v1/marketplace` |
//! |---|---|
//! | `marketplace-http` | 404 |
//! | `managed_resources` | boot refused (unmet `Arc<ManagedRepository>`) |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const MARKETPLACE_HTTP_EXTENSION: &str = "marketplace-http";

/// The member that contributes the marketplace route set.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarketplaceHttpExtension;

impl Extension for MarketplaceHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: MARKETPLACE_HTTP_EXTENSION,
            name: "Marketplace HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(feature = "http"),
        expect(
            unused_variables,
            reason = "the member contributes its routes only with the http feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        routes::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use std::sync::Arc;

    use systemprompt_extension::prelude::*;
    use systemprompt_marketplace::MarketplaceCache;
    use systemprompt_marketplace::managed::ManagedRepository;

    use crate::http::public_routes;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<ManagedRepository>>()
            .requires::<Arc<MarketplaceCache>>()
            .routes(public_routes());
    }
}
