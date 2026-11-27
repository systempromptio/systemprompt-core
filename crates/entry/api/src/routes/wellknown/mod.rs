//! `/.well-known/*` endpoints owned by the API: the federated-auth JWKS
//! document, the bridge discovery document and the two app association
//! documents.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod android;
pub mod apple;
pub mod bridge;
pub mod jwks;

use axum::Router;
use systemprompt_extension::LoaderError;
use systemprompt_runtime::AppContext;

pub const ASSOCIATION_CACHE_CONTROL: &str = "public, max-age=3600";

const ASSOCIATION_RATE_LIMIT_PER_SECOND: u64 = 5;

pub fn wellknown_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    Ok(jwks::jwks_router(ctx)?
        .merge(bridge::bridge_discovery_router(ctx)?)
        .merge(apple::apple_app_site_association_router(ctx)?)
        .merge(android::asset_links_router(ctx)?))
}
