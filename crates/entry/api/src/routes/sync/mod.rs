use axum::Router;
use axum::routing::get;
use systemprompt_runtime::AppContext;

mod files;
mod types;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/files", get(files::download).post(files::upload))
        .route("/files/manifest", get(files::manifest))
}
