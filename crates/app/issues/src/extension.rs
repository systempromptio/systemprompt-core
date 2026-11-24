//! [`IssuesExtension`] (`issues`): the issue report triage commands.
//!
//! With `cli` it contributes the `admin issues` group under the kernel's
//! `admin` group, requesting the caller session (the triager a transition
//! records). The commands read the issue service from the
//! `DocumentsHandlesSlot` the `documents` member provides, which this
//! member `requires`.
//!
//! | profile disables | CLI |
//! |---|---|
//! | `issues` | `admin issues` refused at dispatch |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const ISSUES_EXTENSION: &str = "issues";

/// The member that contributes the issue triage commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct IssuesExtension;

impl Extension for IssuesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: ISSUES_EXTENSION,
            name: "Issues",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(feature = "cli"),
        expect(
            unused_variables,
            reason = "the member contributes its commands only with the cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "cli")]
mod commands {
    use systemprompt_extension::cli::{CommandClaim, CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_files::documents::DocumentsHandlesSlot;

    use crate::commands::IssuesCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<DocumentsHandlesSlot>().command(
            CommandDecl::group_under::<parents::Admin, IssuesCommands>()
                .requesting(&[KernelGrant::CallerSession])
                .claiming(&[CommandClaim::PreservesData]),
        );
    }
}
