//! `WebAuthn` account-link HTML page endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::{Html, IntoResponse};
use systemprompt_oauth::services::TemplateService;

pub async fn link_passkey_page() -> impl IntoResponse {
    Html(TemplateService::load_link_passkey_template())
}
