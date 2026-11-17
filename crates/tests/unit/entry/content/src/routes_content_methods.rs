//! The public and signed-in content route sets share `/api/v1/content`, and
//! the signed-in set holds only `POST /links/generate`. Mounted together, as
//! the API mount merges a base's sets, a `GET` there matches the static
//! generate route rather than the public `/{source_id}/{slug}` route and is
//! refused with 405 (T12); the `POST` reaches the generate handler.

use axum::Extension;
use systemprompt_content_entry::http as content;
use tower::ServiceExt;

use crate::common::{content_app, empty_get, json_post, request_context, setup_ctx};

#[tokio::test]
async fn get_on_links_generate_is_method_not_allowed() -> anyhow::Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let app = content_app(content::public_routes(), &ctx)
        .merge(content_app(content::authenticated_routes(), &ctx))
        .layer(Extension(request_context("content_methods")));

    let get = app.clone().oneshot(empty_get("/links/generate")).await?;
    assert_eq!(get.status().as_u16(), 405, "{}", get.status());

    let post = app
        .oneshot(json_post(
            "/links/generate",
            serde_json::json!({
                "target_url": "https://example.com/methods",
                "link_type": "redirect",
            }),
        ))
        .await?;
    assert_ne!(post.status().as_u16(), 405, "{}", post.status());
    assert_ne!(post.status().as_u16(), 404, "{}", post.status());
    Ok(())
}
