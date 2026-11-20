//! `systemprompt-marketplace-entry` — the marketplace entry surface.
//!
//! The `http` feature carries the capability's routes: the marketplace
//! catalogue and plugin files under `/api/v1/marketplace`.
//! [`extension::MarketplaceHttpExtension`] contributes them to the
//! composition, so the API server does not name this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod extension;
#[cfg(feature = "http")]
pub mod http;
