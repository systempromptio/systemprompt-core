//! Rendering behavior of `ListItemsCardRenderer`.

use serde_json::{Value, json};
use systemprompt_content::ListItemsCardRenderer;
use systemprompt_provider_contracts::{ComponentContext, ComponentRenderer};
use systemprompt_test_fixtures::web_config;

async fn render(items: &[Value]) -> String {
    let wc = web_config();
    let ctx = ComponentContext::for_list(&wc, items);
    ListItemsCardRenderer
        .render(&ctx)
        .await
        .expect("render succeeds")
        .html
}

#[tokio::test]
async fn renders_a_card_with_prefix_title_excerpt_and_date() {
    let html = render(&[json!({
        "content_type": "blog-list",
        "title": "Launch Post",
        "slug": "launch",
        "description": "The launch",
        "published_at": "2026-01-15T12:00:00Z",
    })])
    .await;

    assert!(html.contains(r#"href="/blog/launch""#), "got: {html}");
    assert!(html.contains("<h2 class=\"card-title\">Launch Post</h2>"));
    assert!(html.contains("The launch"));
    assert!(html.contains("January 15, 2026"));
}

#[tokio::test]
async fn item_without_slug_is_skipped_but_valid_siblings_render() {
    let html = render(&[
        json!({"content_type": "news-list", "title": "No Slug"}),
        json!({"content_type": "news-list", "title": "Good", "slug": "good"}),
    ])
    .await;

    assert!(!html.contains("No Slug"));
    assert!(html.contains(r#"href="/news/good""#), "got: {html}");
}

#[tokio::test]
async fn empty_image_falls_back_to_the_placeholder_and_real_image_is_embedded() {
    let with_placeholder = render(&[json!({
        "content_type": "pages-list", "title": "A", "slug": "a", "image": "",
    })])
    .await;
    assert!(with_placeholder.contains("card-image--placeholder"));

    let with_image = render(&[json!({
        "content_type": "pages-list", "title": "B", "slug": "b",
        "image": "/img/b.png",
    })])
    .await;
    assert!(
        with_image.contains(r#"<img src="/img/b.png" alt="B" loading="lazy" />"#),
        "got: {with_image}"
    );
}

#[tokio::test]
async fn unparseable_published_at_is_discarded_rather_than_rendered_raw() {
    let html = render(&[json!({
        "content_type": "blog-list", "title": "A", "slug": "a",
        "published_at": "not-a-date",
    })])
    .await;

    assert!(html.contains(r#"<time class="card-date"></time>"#), "got: {html}");
    assert!(!html.contains("not-a-date"));
}

#[tokio::test]
async fn content_type_without_list_suffix_is_used_verbatim_as_prefix() {
    let html = render(&[json!({
        "content_type": "docs", "title": "A", "slug": "a",
    })])
    .await;

    assert!(html.contains(r#"href="/docs/a""#), "got: {html}");
}

#[tokio::test]
async fn missing_items_render_to_an_empty_component() {
    let wc = web_config();
    let ctx = ComponentContext::for_page(&wc);
    let rendered = ListItemsCardRenderer
        .render(&ctx)
        .await
        .expect("render succeeds");

    assert_eq!(rendered.html, "");
}
