//! [`ContentHttpExtension`] (`content-http`): the content route sets and the
//! content commands.
//!
//! With `http` it contributes three route sets, all in the content-and-misc
//! role group under the `content` rate-limit scope and budget:
//!
//! - `/api/v1/content` public reads behind the public gate with a public
//!   request context;
//! - `/api/v1/content` link generation (`POST /links/generate`) behind the user
//!   gate with a user request context;
//! - the short-link redirect `/r/{short_code}` at the web root behind the
//!   public gate with a public request context.
//!
//! Each set's shape equals the reference composition's conferral for
//! `content-http` at its base. The public and signed-in sets share their
//! base, so the conferral pins routes instead: the signed-in set holds only
//! `POST /links/generate`, and the redirect set only `GET /r/{short_code}`.
//! The handlers read the content repositories the `content` member provides,
//! which this member `requires`.
//!
//! With `cli` it contributes the `core content` group, which the `popular`
//! leaf of `analytics-http` attaches under, and the `web sitemap` group.
//!
//! | profile disables | public | authenticated | redirect | CLI |
//! |---|---|---|---|---|
//! | `content-http` | 404 | 404 | 404 | refused at dispatch |
//! | `content` | boot refused (unmet `Arc<ContentRepositories>`) | | | |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const CONTENT_HTTP_EXTENSION: &str = "content-http";

/// The member that contributes the content route sets and commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContentHttpExtension;

impl Extension for ContentHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: CONTENT_HTTP_EXTENSION,
            name: "Content HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

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

    use systemprompt_content::repository::ContentRepositories;
    use systemprompt_extension::prelude::*;

    use crate::http::{authenticated_routes, public_routes, redirect_routes};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<ContentRepositories>>()
            .routes(public_routes())
            .routes(authenticated_routes())
            .routes(redirect_routes());
    }
}

#[cfg(feature = "cli")]
mod commands {
    use systemprompt_extension::cli::{CommandDecl, parents};
    use systemprompt_extension::prelude::*;

    use crate::cli::content::ContentCommands;
    use crate::cli::sitemap::SitemapCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.command(CommandDecl::group_under::<parents::Core, ContentCommands>())
            .command(CommandDecl::group_under::<parents::Web, SitemapCommands>());
    }
}
