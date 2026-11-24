//! [`FilesHttpExtension`] (`files-http`): the document store's HTTP surface
//! and the files commands.
//!
//! With `http` it contributes the open
//! [`DynFilesSurface`](systemprompt_documents::http::surface::DynFilesSurface)
//! the API mount builds the document, upload, public-link and issue routes
//! from (see [`crate::http`]), the open
//! [`DynDriveSurface`](systemprompt_documents::http::surface::DynDriveSurface)
//! the bridge Drive feed is built from, and the open
//! [`FilesHttpSettings`](crate::http::FilesHttpSettings) it copies out of the
//! application configuration, and provides sealed the
//! [`FilesHttpRepositories`](crate::http::FilesHttpRepositories) those routes
//! read. The API keeps their layering: the two-stage token and user gates of
//! documents and uploads, the policies and the rate limits.
//!
//! With `cli` it contributes the `core files` group under the kernel's
//! `core` group. The commands resolve the `Arc<FileRepository>` the `files`
//! member provides, which this member `requires`, and the kernel's open
//! `Arc<FilesConfig>` and `Arc<dyn FileStorage>` seeds. `/api/v1/files`
//! stays reserved to the kernel and the `/files/*` static fallback is
//! kernel-served.
//!
//! | profile disables | document, upload, public-link, issue and bridge Drive routes | CLI |
//! |---|---|---|
//! | `files-http` | 404 | `core files` refused at dispatch |
//! | `files` | boot refused (`files` is a required member) | |
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
        not(any(feature = "http", feature = "cli")),
        expect(
            unused_variables,
            reason = "the member contributes its surface only with the http feature and its commands only with the cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        surface::contribute(c);
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod surface {
    use std::sync::Arc;

    use systemprompt_database::DbPool;
    use systemprompt_documents::http::surface::{DynDriveSurface, DynFilesSurface};
    use systemprompt_extension::prelude::*;
    use systemprompt_users::UserRepository;

    use systemprompt_manifest::Config;

    use crate::http::drive::FilesDriveSurface;
    use crate::http::{FilesHttpRepositories, FilesHttpSettings, FilesHttpSurface};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.provide(
            |_r: &Resolved<'_>| -> Result<DynFilesSurface, ProvideError> {
                Ok(Arc::new(FilesHttpSurface))
            },
        )
        .provide(
            |_r: &Resolved<'_>| -> Result<DynDriveSurface, ProvideError> {
                Ok(Arc::new(FilesDriveSurface))
            },
        )
        .provide(
            |r: &Resolved<'_>| -> Result<FilesHttpSettings, ProvideError> {
                let config = r.get::<Arc<Config>>()?;
                Ok(FilesHttpSettings::of(&config))
            },
        )
        .provide_sealed(repositories);
    }

    fn repositories(r: &Resolved<'_>) -> Result<Arc<FilesHttpRepositories>, ProvideError> {
        let db = r.get::<DbPool>()?;
        Ok(Arc::new(FilesHttpRepositories {
            users: Arc::new(UserRepository::new(&db)),
        }))
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
        c.requires::<Arc<FileRepository>>().command(
            CommandDecl::group_under::<parents::Core, FilesCommands>()
                .claiming(&[CommandClaim::PreservesData, CommandClaim::DirectDatabase]),
        );
    }
}
