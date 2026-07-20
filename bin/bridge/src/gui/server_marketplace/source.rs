//! External marketplace-source contribution point.
//!
//! White-label crates register a [`MarketplaceSource`] via
//! [`register_marketplace_source!`](crate::register_marketplace_source) to
//! inject branded items into any of the six marketplace categories, without
//! editing core's built-in scanners. Sources are consulted during
//! [`super::build_listing`]. A source carries a `priority` (default 0):
//! higher-priority source items are merged first, and each category is deduped
//! by item id keeping the first-seen — so a source at `priority > 0` can
//! **shadow a built-in item** of the same id, not only append new ones.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::proxy::mcp_probe::McpServerAuth;

use super::MarketplaceItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceCategory {
    Plugins,
    Skills,
    Hooks,
    Mcp,
    Agents,
    Artifacts,
}

#[derive(Debug)]
pub struct MarketplaceSourceCtx<'a> {
    pub plugins_root: Option<&'a Path>,
    pub mcp_auth: &'a [McpServerAuth],
}

pub trait MarketplaceSource: Sync {
    fn category(&self) -> MarketplaceCategory;
    fn items(&self, ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem>;
}

#[derive(Clone, Copy)]
pub struct MarketplaceSourceRegistration {
    pub source: &'static dyn MarketplaceSource,
    pub priority: i32,
}

impl std::fmt::Debug for MarketplaceSourceRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceSourceRegistration")
            .field("category", &self.source.category())
            .field("priority", &self.priority)
            .finish()
    }
}

inventory::collect!(MarketplaceSourceRegistration);

/// Register a [`MarketplaceSource`] into the compile-time marketplace registry.
/// An optional `priority = N` (default 0) makes this source's items shadow a
/// built-in item of the same id within its category.
#[macro_export]
macro_rules! register_marketplace_source {
    ($e:expr, priority = $p:expr $(,)?) => {
        ::inventory::submit! {
            $crate::gui::server_marketplace::source::MarketplaceSourceRegistration { source: &$e, priority: $p }
        }
    };
    ($e:expr $(,)?) => {
        $crate::register_marketplace_source!($e, priority = 0);
    };
}
