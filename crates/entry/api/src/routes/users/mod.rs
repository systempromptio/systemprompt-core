//! Self-service user account routes scoped to the authenticated caller.
//!
//! Hosts the `/me` endpoints, including session revocation in [`sessions`].

pub mod sessions;

use axum::Router;
use axum::routing::post;
use systemprompt_runtime::AppContext;

pub fn router(ctx: &AppContext) -> Router {
    Router::new()
        .route("/me/sessions/revoke_all", post(sessions::revoke_all_mine))
        .with_state(ctx.clone())
}
