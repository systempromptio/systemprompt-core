//! Cowork desktop marketplace + plugin store integration.
//!
//! Cowork loads plugins via a per-session marketplace registry under
//! `<session>/<org>/cowork_plugins/`. The bridge registers a single
//! `systemprompt-bridge-managed` marketplace there with `source: "local"`,
//! pointing at on-disk plugin content the bridge controls. This is the proper
//! marketplace mechanism (survives restart) — not a manifest patch.
//!
//! Pure data manipulation lives in the `registry`, `settings` and
//! `marketplace` submodules; IO (`emit`) and per-file upsert plumbing layer
//! on top.

pub(crate) mod emit;
pub(crate) mod marketplace;
pub(crate) mod registry;
pub(crate) mod settings;
mod upsert;

pub use emit::{publish, resolve_target, unpublish};

pub use marketplace::{
    MarketplaceFile, MarketplaceOwner, MarketplacePluginEntry, render_marketplace,
};
pub use registry::{
    InstalledPluginEntry, KnownMarketplaceEntry, LocalSource, MergeReport, parse_root,
    upsert_installed_plugin, upsert_known_marketplace,
};
pub use settings::{
    SettingsReport, disable_plugin, enable_plugin, enabled_plugins_key, parse_settings,
    render_settings,
};

// Lives outside `cowork_plugins/` (in `<session>/<org>/`) — Cowork resolves
// `enabledPlugins` from there, not from inside the registry tree.
pub(crate) const COWORK_SETTINGS_FILE: &str = "cowork_settings.json";

use thiserror::Error;

pub const KNOWN_MARKETPLACES_FILE: &str = "known_marketplaces.json";

pub(crate) const INSTALLED_PLUGINS_FILE: &str = "installed_plugins.json";

#[derive(Debug, Error)]
pub enum CoworkPluginsError {
    #[error("json parse failed: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("registry root must be a JSON object")]
    RootShape,
    #[error("registry items at key `{key}` must be a JSON array")]
    ItemsShape { key: &'static str },
}
