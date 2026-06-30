//! # systemprompt-marketplace
//!
//! The marketplace bounded context: resolving which marketplace is active,
//! loading the on-disk catalogue, scoping and per-user filtering it, and
//! assembling the canonical signed bridge manifest.
//!
//! ## Public surface
//!
//! - [`MarketplaceService`]: read-only resolution over a borrowed
//!   `ServicesConfig` (lookup, default fallback, active marketplace,
//!   referential-integrity check).
//! - [`catalog`]: on-disk loaders projecting the services tree into the signed
//!   `*Entry` records the manifest carries. [`CatalogContent`] owns the
//!   resolved catalogue; [`plugin_bundles`] is the single source of the active,
//!   content-gated plugin bundles shared by the manifest and serving paths.
//! - [`bundle`]: the build-from-spec plugin-bundle assembler
//!   ([`build_plugin_bundle`]) — the single owner of the `.claude-plugin`
//!   bundle contract, consumed by both the manifest and byte-serving paths.
//! - [`scope_to_marketplace`] / [`active_marketplace`]: marketplace scoping of
//!   the catalogue lists.
//! - [`ManifestService`] / [`CanonicalView`]: assemble a scoped, filtered
//!   [`MarketplaceCandidate`] and sign the canonical view.
//! - [`MarketplaceFilter`] / [`MarketplaceCandidate`] / [`AllowAllFilter`]: the
//!   per-user filtering contract applied before signing.
//! - [`render_marketplace_json`] / [`render_marketplace_list`]: JSON
//!   projections for the HTTP catalogue endpoints.
//! - [`MarketplaceFilterRegistration`] / [`discover_filters`]: the inventory
//!   slot and lookup used to wire an extension-supplied filter.
//!
//! ## Error model
//!
//! [`MarketplaceError`] is the crate-wide error for fallible services
//! (lookup, catalogue load, signing); [`MarketplaceFilterError`] is the
//! narrower error a filter implementation returns and folds into it.
//!
//! ## Layer
//!
//! Domain crate. Depends on `systemprompt-models` (wire types),
//! `systemprompt-identifiers` (typed IDs), `systemprompt-database` (the
//! `DbPool` handle passed to filter factories), and `systemprompt-security`
//! (manifest signing). No HTTP and no database queries: loaders take a
//! services-root path, never an `AppContext`.

pub mod bundle;
mod candidate;
pub mod catalog;
mod error;
mod filter;
mod manifest;
mod registry;
mod scope;
mod service;
mod view;

pub use bundle::{
    BundleContent, BundleFile, PluginBundle, build_plugin_bundle, bundle_has_content,
};
pub use candidate::MarketplaceCandidate;
pub use catalog::{CatalogContent, plugin_bundles, plugin_bundles_cached};
pub use error::{MarketplaceError, MarketplaceFilterError};
pub use filter::{AllowAllFilter, MarketplaceFilter};
pub use manifest::{CanonicalView, ManifestService};
pub use registry::{MarketplaceFilterRegistration, discover_filters};
pub use scope::{active_marketplace, scope_to_marketplace};
pub use service::MarketplaceService;
pub use view::{render_marketplace_json, render_marketplace_list};
