use axum::response::{Html, IntoResponse};
use systemprompt_oauth::services::TemplateEngine;

#[expect(clippy::unused_async)]
pub async fn link_passkey_page() -> impl IntoResponse {
    Html(TemplateEngine::load_link_passkey_template())
}
