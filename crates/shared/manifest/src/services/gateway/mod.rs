//! Gateway services section: the one default model and the policy routes.
//!
//! Lives in the services tree (`services/ai/gateway.yaml` by convention)
//! beside the catalog it references, not in the profile.
//!
//! - [`GatewayProfileError`] / [`GatewayResult`] — failure modes of the section
//!   on its own (route ids and shape, predicates, `[1m]` refusal).
//! - [`GatewayConfigSpec`] — the on-disk shape under `gateway:` in a services
//!   YAML document.
//! - [`GatewayConfig`] — the runtime shape produced by
//!   [`GatewayConfigSpec::resolve`]. The gateway owns no catalog: routes and
//!   `default_model` are resolved against the catalog by
//!   [`ResolvedCatalog`](crate::services::catalog::ResolvedCatalog).
//! - [`GatewayRoute`] — a policy route with a declared, stable id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod error;
mod override_rule;
mod route;
mod route_id;
mod state;

pub use config::{BridgeReleasesSpec, GatewayConfig, GatewayConfigSpec, QuotaFaultMode};
pub use error::{GatewayProfileError, GatewayResult};
pub use override_rule::{OverrideRuleAction, SystemPromptRule};
pub use route::{
    GatewayRoute, ModelGlobs, ResponseFormatKind, RouteAlias, RouteDeployment, RouteMatch,
    RouteRequirements, SelectionStrategy, WeightedDeployment, estimate_input_tokens,
};
pub use route_id::{slugify_pattern, synthesize_route_id};
pub use state::GatewayState;
