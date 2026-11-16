//! The cloud member: behind the `cli` feature, the top-level `cloud` command
//! group with the `CloudOperator` grant the reserved `cloud` path confers.
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
        not(feature = "cli"),
        expect(unused_variables, reason = "the cloud member contributes only CLI commands")
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "cli")]
        c.command(
            systemprompt_extension::cli::CommandDecl::group::<crate::cli::CloudCommands>()
                .requesting(&[
                    systemprompt_extension::cli::KernelGrant::CloudOperator,
                    systemprompt_extension::cli::KernelGrant::BootProfile,
                ]),
        );
        Ok(())
    }
}
