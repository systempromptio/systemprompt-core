use axum::response::{Html, IntoResponse};
use systemprompt_oauth::services::TemplateEngine;

// reason: Axum handler signature requires `async fn` even when the body has no
// await points
#[allow(clippy::unused_async)]
pub async fn link_passkey_page() -> impl IntoResponse {
    Html(TemplateEngine::load_link_passkey_template())
}
