//! `/.well-known/*` endpoints owned by the API: A2A agent cards, the
//! federated-auth JWKS document and the bridge discovery document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_cards;
pub mod bridge;
pub mod jwks;

use axum::Router;
use systemprompt_extension::LoaderError;
use systemprompt_runtime::AppContext;

pub fn wellknown_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    Ok(agent_cards::wellknown_router(ctx)
        .merge(jwks::jwks_router(ctx)?)
        .merge(bridge::bridge_discovery_router(ctx)?))
}
