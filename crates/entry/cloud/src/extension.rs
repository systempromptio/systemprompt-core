//! The cloud member's contributions.
//!
//! Behind the `cli` feature, the top-level `cloud` command group with the
//! `CloudOperator` grant the reserved `cloud` path confers; behind the `http`
//! feature, the open `DynSyncSurface` the API builds the services-tree sync
//! routes from (`cloud` disabled: `/api/v1/sync` is 404).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct CloudCliExtension;

impl Extension for CloudCliExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "cloud",
            name: "Cloud",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(any(feature = "cli", feature = "http")),
        expect(
            unused_variables,
            reason = "the cloud member contributes only with the cli or http feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        c.provide(
            |_r: &Resolved<'_>| -> Result<systemprompt_extension::router::DynSyncSurface, ProvideError> {
                Ok(std::sync::Arc::new(crate::http::CloudSyncSurface))
            },
        );
        #[cfg(feature = "cli")]
        c.command(
            systemprompt_extension::cli::CommandDecl::group::<crate::cli::CloudCommands>()
                .requesting(&[
                    systemprompt_extension::cli::KernelGrant::CloudOperator,
                    systemprompt_extension::cli::KernelGrant::BootProfile,
                    systemprompt_extension::cli::KernelGrant::ConfigReconcile,
                    systemprompt_extension::cli::KernelGrant::TargetAdmission,
                ])
                .claiming(&[
                    systemprompt_extension::cli::CommandClaim::ReadOnlyRouting,
                    systemprompt_extension::cli::CommandClaim::PreservesData,
                ]),
        );
        Ok(())
    }
}
