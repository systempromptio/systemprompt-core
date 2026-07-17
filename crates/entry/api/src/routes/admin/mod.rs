//! Admin-only routes mounted under the gateway's authenticated admin scope.
//!
//! Composes the CLI gateway (`cli`) and API-key management (`keys`)
//! sub-routers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cli;
mod keys;

use axum::Router;
use systemprompt_runtime::AppContext;

#[cfg(feature = "test-api")]
pub use cli::test_api as cli_test_api;

pub fn router() -> Router<AppContext> {
    Router::new()
        .nest("/cli", cli::router())
        .nest("/api-keys", keys::router())
}
