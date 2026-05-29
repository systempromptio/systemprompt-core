//! Admin-only routes mounted under the gateway's authenticated admin scope.
//!
//! Composes the CLI gateway (`cli`) and API-key management (`keys`)
//! sub-routers.

mod cli;
mod keys;

use axum::Router;
use systemprompt_runtime::AppContext;

pub fn router() -> Router<AppContext> {
    Router::new()
        .nest("/cli", cli::router())
        .nest("/api-keys", keys::router())
}
