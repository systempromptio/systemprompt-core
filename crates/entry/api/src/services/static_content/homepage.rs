use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use systemprompt_models::AppPaths;

pub async fn serve_homepage() -> impl IntoResponse {
    let dist_dir = match AppPaths::get() {
        Ok(paths) => paths.web().dist().to_path_buf(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AppPaths not initialized",
            )
                .into_response();
        },
    };

    let homepage_path = dist_dir.join("index.html");

    if homepage_path.exists() {
        match std::fs::read(&homepage_path) {
            Ok(content) => {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    content,
                )
                    .into_response();
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to read homepage");
                return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading homepage")
                    .into_response();
            },
        }
    }

    (
        StatusCode::TEMPORARY_REDIRECT,
        [(header::LOCATION, "/agent/")],
        "",
    )
        .into_response()
}
