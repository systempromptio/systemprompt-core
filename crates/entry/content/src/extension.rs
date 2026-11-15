//! [`ContentHttpExtension`]: declares the three content route groups.
//!
//! All three sit in the content-and-misc role group under the `content`
//! rate-limit scope and budget:
//!
//! - `/api/v1/content` public reads behind the public gate with a public
//!   request context;
//! - `/api/v1/content` link generation (`POST /links/generate`) behind the user
//!   gate with a user request context;
//! - the short-link redirect `/r/{short_code}` at the web root behind the
//!   public gate with a public request context.
//!
//! Each declaration matches its row of the API's closed core table
//! (`ContentPublic`, `ContentAuthenticated`, `ContentRedirect`), keyed on this
//! extension's id, `content-http`. The handlers read the content repositories
//! the `content` extension provides; this extension `requires` them:
//!
//! | profile disables | public | authenticated | redirect |
//! |---|---|---|---|
//! | `content-http` | 404 | 404 | 404 |
//! | `content` | boot refused (unmet `Arc<ContentRepositories>`) | | |
//! | both | 404 | 404 | 404 |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_content::repository::ContentRepositories;
use systemprompt_extension::RouteGroup;
use systemprompt_extension::prelude::*;
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_models::modules::ApiPaths;

use crate::http::{ContentHttpState, authenticated_router, public_router, redirect_router};

pub const CONTENT_HTTP_EXTENSION: &str = "content-http";

const RATE_LIMIT: RateLimit = RateLimit::new("content", RateBudget::Content);

/// The extension that declares the content route groups.
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

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["content"]
    }

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<Arc<ContentRepositories>>()]
    }

    fn routers(
        &self,
        ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        let state =
            ContentHttpState::from_extension_state(&ctx.extension_state(), &ctx.app_config())?;
        Ok(vec![
            ExtensionRouter::new(
                public_router(state.clone()),
                ApiPaths::CONTENT_BASE,
                AuthzPolicy::public(),
                ContextKind::Public,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
            ExtensionRouter::new(
                authenticated_router(state.clone()),
                ApiPaths::CONTENT_BASE,
                AuthzPolicy::user(),
                ContextKind::User,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
            ExtensionRouter::new(
                redirect_router(state),
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
        ])
    }
}

register_extension!(ContentHttpExtension);
