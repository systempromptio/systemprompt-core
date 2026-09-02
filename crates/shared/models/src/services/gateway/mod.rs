//! Gateway services section: routes that map external model names onto entries
//! in the services provider registry.
//!
//! Carries the cross-check that ties the two together. Lives in the services
//! tree (`services/ai/gateway.yaml` by convention) beside the catalog it
//! references, not in the profile.
//!
//! - [`GatewayProfileError`] / [`GatewayResult`] — failure modes emitted by
//!   route-id and provider-reference validation.
//! - [`GatewayConfigSpec`] — the on-disk shape under `gateway:` in a services
//!   YAML document.
//! - [`GatewayConfig`] — the runtime shape produced by
//!   [`GatewayConfigSpec::resolve`]. The gateway owns no catalog: every route
//!   resolves its provider against `services.providers` (the merged
//!   `providers:` list of the services tree) at use time.
//! - [`GatewayRoute`] — routing patterns and the stable id synthesis used to
//!   address routes from `access_control_rules`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod error;
mod override_rule;
mod route;
mod state;

pub use config::{BridgeReleasesSpec, GatewayConfig, GatewayConfigSpec};
pub use error::{GatewayProfileError, GatewayResult};
pub use override_rule::{OverrideRuleAction, SystemPromptRule};
pub use route::{
    GatewayRoute, ResponseFormatKind, RouteMatch, RouteRequirements, slugify_pattern,
    synthesize_route_id,
};
pub use state::GatewayState;
