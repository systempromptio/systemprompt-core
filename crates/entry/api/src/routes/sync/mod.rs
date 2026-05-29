//! Cloud-sync routes for transferring the `services/` configuration tree.
//!
//! Mounts the file manifest, download, and upload handlers from [`files`] used
//! to push and pull agent, skill, content, and config definitions.

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
