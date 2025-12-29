use axum::routing::{get, post};
use axum::Router;
use systemprompt_runtime::AppContext;

mod export;
mod import;
mod types;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/database/export", get(export::export))
        .route("/database/import", post(import::import))
}
