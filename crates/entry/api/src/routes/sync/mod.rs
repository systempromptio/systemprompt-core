use axum::middleware::from_fn;
use axum::routing::get;
use axum::Router;
use systemprompt_runtime::AppContext;

mod auth;
mod files;
mod types;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/files", get(files::download).post(files::upload))
        .route("/files/manifest", get(files::manifest))
        .layer(from_fn(auth::sync_token_middleware))
}
