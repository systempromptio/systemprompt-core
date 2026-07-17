//! `/.well-known/*` endpoints owned by the API: A2A agent cards plus the
//! federated-auth JWKS document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_cards;
pub mod jwks;

use axum::Router;
use systemprompt_runtime::AppContext;

pub use agent_cards::wellknown_router as agent_card_router;

pub fn wellknown_router(ctx: &AppContext) -> Router {
    agent_cards::wellknown_router(ctx).merge(jwks::jwks_router(ctx))
}
