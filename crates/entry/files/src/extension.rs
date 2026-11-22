//! [`FilesHttpExtension`] (`files-http`): the files commands.
//!
//! With `cli` it contributes the `core files` group under the kernel's
//! `core` group. The commands resolve the `Arc<FileRepository>` the `files`
//! member provides, which this member `requires`, and the kernel's open
//! `Arc<FilesConfig>` and `Arc<dyn FileStorage>` seeds. It contributes no
//! route set: `/api/v1/files` stays reserved to the kernel and the
//! `/files/*` static fallback is kernel-served.
//!
//! | profile disables | CLI |
//! |---|---|
//! | `files-http` | `core files` refused at dispatch |
//! | `files` | boot refused (`files` is a required member) |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const FILES_HTTP_EXTENSION: &str = "files-http";

/// The member that contributes the files commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilesHttpExtension;

impl Extension for FilesHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: FILES_HTTP_EXTENSION,
            name: "Files HTTP",
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
    use std::sync::Arc;

    use systemprompt_extension::cli::{CommandClaim, CommandDecl, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_files::FileRepository;

    use crate::cli::FilesCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<FileRepository>>()
            .command(
                CommandDecl::group_under::<parents::Core, FilesCommands>()
                    .claiming(&[CommandClaim::PreservesData, CommandClaim::DirectDatabase]),
            );
    }
}
