//! `systemprompt-marketplace-entry` — the marketplace entry surface.
//!
//! The `http` feature carries the capability's routes: the marketplace
//! catalogue and plugin files under `/api/v1/marketplace`. The `cli` feature
//! carries its commands: `core marketplace`, `core plugins` and
//! `core skills`. [`extension::MarketplaceHttpExtension`] contributes both
//! to the composition, so neither the API server nor the CLI kernel names
//! this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
