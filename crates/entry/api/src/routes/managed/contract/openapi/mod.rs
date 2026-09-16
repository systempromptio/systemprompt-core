//! `OpenAPI` 3.1 schemas are generated from the same DTOs used by handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
mod analytics;
mod builder;
mod consumers;
mod inventory;
mod resources;
use builder::Document;

// JSON: generated OpenAPI is a schema document; endpoint DTOs remain typed.
pub fn document() -> serde_json::Value {
    let mut document = Document::new();
    consumers::register(&mut document);
    inventory::register(&mut document);
    analytics::register(&mut document);
    resources::register(&mut document);
    document.finish()
}
pub async fn serve() -> axum::Json<serde_json::Value> {
    axum::Json(document())
}
