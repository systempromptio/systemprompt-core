//! Gateway profile section: model catalog, routes, and the cross-check that
//! ties them together.
//!
//! - [`error`] / [`GatewayProfileError`] / [`GatewayResult`] — all failure
//!   modes emitted by catalog load, validation, and route synthesis.
//! - [`config`] / [`GatewayConfig`] — the top-level section embedded in a
//!   profile, with [`GatewayConfig::validate`] enforcing catalog/route
//!   consistency.
//! - [`catalog`] / [`GatewayCatalog`] — providers + models, with the
//!   SSRF-hardened endpoint guard.
//! - [`route`] / [`GatewayRoute`] — routing patterns and the stable id
//!   synthesis used to address routes from `access_control_rules`.

mod catalog;
mod config;
mod error;
mod route;

pub use catalog::{GatewayCatalog, GatewayModel, GatewayProvider};
pub use config::GatewayConfig;
pub use error::{GatewayProfileError, GatewayResult};
pub use route::{GatewayRoute, slugify_pattern, synthesize_route_id};
