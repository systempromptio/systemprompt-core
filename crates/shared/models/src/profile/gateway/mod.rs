//! Gateway profile section: model catalog, routes, and the cross-check that
//! ties them together.
//!
//! - [`error`] / [`GatewayProfileError`] / [`GatewayResult`] — all failure
//!   modes emitted by catalog load, validation, and route synthesis.
//! - [`config`] / [`GatewayConfigSpec`] — the on-disk shape embedded in a
//!   profile YAML, with [`GatewayCatalogSource`] selecting inline or
//!   file-backed catalog.
//! - [`config`] / [`GatewayConfig`] — the runtime shape produced by
//!   [`GatewayConfigSpec::resolve`], with the catalog fully loaded and
//!   validated.
//! - [`catalog`] / [`GatewayCatalog`] — providers + models, with the
//!   SSRF-hardened endpoint guard.
//! - [`route`] / [`GatewayRoute`] — routing patterns and the stable id
//!   synthesis used to address routes from `access_control_rules`.

mod catalog;
mod config;
mod error;
mod route;
mod state;

pub use catalog::{GatewayCatalog, GatewayModel, GatewayProvider};
pub use config::{GatewayCatalogSource, GatewayConfig, GatewayConfigSpec};
pub use error::{GatewayProfileError, GatewayResult};
pub use route::{GatewayRoute, slugify_pattern, synthesize_route_id};
pub use state::GatewayState;
