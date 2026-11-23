//! Admin-only routes mounted under the gateway's authenticated admin scope.
//!
//! Composes the CLI gateway (`cli`) and services bundle (`services`)
//! sub-routers. API-key management is the `identity-http` member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cli;
pub mod services;

use axum::Router;
use systemprompt_runtime::AppContext;

pub fn router() -> Router<AppContext> {
    Router::new()
        .nest("/cli", cli::router())
        .nest("/services", services::router())
}
