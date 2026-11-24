//! File-download routes for the `services/` configuration tree.
//!
//! Mounts the file manifest and download handlers from `files` used by
//! `systemprompt cloud backup` to pull agent, skill, content, and config
//! definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::get;
use systemprompt_runtime::AppContext;

mod archive;
mod files;
mod types;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/files", get(files::download))
        .route("/files/manifest", get(files::manifest))
}
