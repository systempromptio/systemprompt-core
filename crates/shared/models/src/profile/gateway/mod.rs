//! Gateway profile section: routes that map external model names onto entries
//! in `profile.providers`, plus the cross-check that ties them together.
//!
//! - [`error`] / [`GatewayProfileError`] / [`GatewayResult`] — failure modes
//!   emitted by route-id and provider-reference validation.
//! - [`config`] / [`GatewayConfigSpec`] — the on-disk shape embedded in a
//!   profile YAML document.
//! - [`config`] / [`GatewayConfig`] — the runtime shape produced by
//!   [`GatewayConfigSpec::resolve`]. The gateway owns no catalog: every route
//!   resolves its provider against `profile.providers` at use time.
//! - [`route`] / [`GatewayRoute`] — routing patterns and the stable id
//!   synthesis used to address routes from `access_control_rules`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod error;
mod override_rule;
mod route;
mod state;

pub use config::{GatewayConfig, GatewayConfigSpec};
pub use error::{GatewayProfileError, GatewayResult};
pub use override_rule::{OverrideRuleAction, SystemPromptRule};
pub use route::{
    GatewayRoute, ResponseFormatKind, RouteMatch, slugify_pattern, synthesize_route_id,
};
pub use state::GatewayState;
