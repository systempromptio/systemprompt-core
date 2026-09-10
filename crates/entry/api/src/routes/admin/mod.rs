//! Admin-only routes mounted under the gateway's authenticated admin scope.
//!
//! Composes the CLI gateway (`cli`), API-key management (`keys`) and services
//! bundle (`services`) sub-routers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cli;
mod keys;
pub mod services;

use axum::Router;
use systemprompt_runtime::AppContext;

pub fn router() -> Router<AppContext> {
    Router::new()
        .nest("/cli", cli::router())
        .nest("/api-keys", keys::router())
        .nest("/services", services::router())
}
