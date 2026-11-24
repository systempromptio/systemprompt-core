//! [`MarketplaceHttpExtension`] (`marketplace-http`): the marketplace route
//! set and the marketplace commands.
//!
//! With `http` it contributes one route set at `/api/v1/marketplace` in the
//! content-and-misc role group, behind the public gate with a public request
//! context, under the `marketplace` rate-limit scope with the content budget
//! and the default body limit. Its shape equals the reference composition's
//! conferral for `marketplace-http` at that base. The handlers read the
//! managed repository and the catalogue cache the `managed_resources` member
//! provides, which this member `requires`, and the
//! [`MarketplaceHttpSettings`](crate::http::MarketplaceHttpSettings) its own
//! provide fn builds from the configuration and the system admin.
//!
//! It also provides the open
//! [`DynManagedSurface`](systemprompt_marketplace::surface::DynManagedSurface)
//! the API builds the device-credential consumer routes under `/api/v1`
//! from; the API keeps their protection (handler-verified by the managed
//! device credential, the `consumer_evidence` limit) between the member's
//! origin guard and contract normaliser. With `marketplace-http` disabled the
//! consumer routes are not mounted (404).
//!
//! With `cli` it contributes the `core marketplace`, `core plugins` and
//! `core skills` groups under the kernel's `core` group, each requesting
//! `KernelGrant::ServicesTree`, the grant their reserved CLI rows list. The
//! `core skills` commands resolve the owner-bound
//! `Arc<OwnerSkillView>` the `managed_resources` member provides.
//!
//! | profile disables | `/api/v1/marketplace` | CLI |
//! |---|---|---|
//! | `marketplace-http` | 404 (consumer routes too) | `core marketplace`, `core plugins`, `core skills` refused at dispatch |
//! | `managed_resources` | boot refused (required member) | boot refused (required member) |
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
        not(any(feature = "http", feature = "cli")),
        expect(
            unused_variables,
            reason = "the member contributes its routes only with the http feature and its commands only with the cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        routes::contribute(c);
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use std::sync::Arc;

    use systemprompt_extension::prelude::*;
    use systemprompt_manifest::Config;
    use systemprompt_manifest::services::SystemAdmin;
    use systemprompt_marketplace::MarketplaceCache;
    use systemprompt_marketplace::managed::ManagedRepository;

    use systemprompt_marketplace::surface::DynManagedSurface;

    use crate::http::{ManagedHttpSurface, MarketplaceHttpSettings, public_routes};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<ManagedRepository>>()
            .requires::<Arc<MarketplaceCache>>()
            .provide(
                |_r: &Resolved<'_>| -> Result<DynManagedSurface, ProvideError> {
                    Ok(Arc::new(ManagedHttpSurface))
                },
            )
            .provide(
                |r: &Resolved<'_>| -> Result<MarketplaceHttpSettings, ProvideError> {
                    let config = r.get::<Arc<Config>>()?;
                    Ok(MarketplaceHttpSettings::of(
                        &config,
                        &r.get::<SystemAdmin>()?,
                    ))
                },
            )
            .routes(public_routes());
    }
}

#[cfg(feature = "cli")]
mod commands {
    use std::sync::Arc;

    use systemprompt_extension::cli::{CommandClaim, CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_marketplace::managed::OwnerSkillView;

    use crate::cli::{MarketplaceCommands, PluginsCommands, SkillsCommands};

    const SERVICES_TREE: &[KernelGrant] = &[KernelGrant::ServicesTree];

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<OwnerSkillView>>()
            .command(
                CommandDecl::group_under::<parents::Core, MarketplaceCommands>()
                    .requesting(SERVICES_TREE)
                    .claiming(&[CommandClaim::PreservesData]),
            )
            .command(
                CommandDecl::group_under::<parents::Core, PluginsCommands>()
                    .requesting(SERVICES_TREE),
            )
            .command(
                CommandDecl::group_under::<parents::Core, SkillsCommands>()
                    .requesting(SERVICES_TREE)
                    .claiming(&[CommandClaim::LocalDatabase, CommandClaim::PreservesData]),
            );
    }
}
