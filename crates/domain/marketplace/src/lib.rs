//! # systemprompt-marketplace
//!
//! Per-user marketplace filtering for the bridge manifest.
//!
//! ## Public surface
//!
//! - [`MarketplaceFilter`]: the trait that gateway handlers invoke to restrict
//!   what a given user sees in `GET /v1/bridge/manifest`.
//! - [`MarketplaceCandidate`]: the bundle of plugins, skills, agents, and
//!   managed MCP servers a filter may keep, drop, or rewrite.
//! - [`AllowAllFilter`]: passthrough default used when no extension registers a
//!   policy.
//! - [`MarketplaceFilterError`]: error type returned by fallible
//!   implementations.
//!
//! ## Layer
//!
//! Domain crate. Depends on `systemprompt-models` (wire types) and
//! `systemprompt-identifiers` only. No database, no HTTP, no async
//! runtime hooks beyond `async-trait`.
//!
//! ## Wiring
//!
//! `AppContext` holds an `Arc<dyn MarketplaceFilter>`. The bridge
//! manifest handler in `crates/entry/api` reads it and applies the
//! filter before assembling the canonical signed view. Deployments
//! plug their own implementation in via the runtime builder.

mod candidate;
mod error;
mod filter;
mod registry;

pub use candidate::MarketplaceCandidate;
pub use error::MarketplaceFilterError;
pub use filter::{AllowAllFilter, MarketplaceFilter};
pub use registry::{MarketplaceFilterRegistration, discover_filters};
