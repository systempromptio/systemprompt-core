//! Admin API routes.

mod cli;

use axum::Router;
use systemprompt_runtime::AppContext;

pub fn router() -> Router<AppContext> {
    Router::new().nest("/cli", cli::router())
}
