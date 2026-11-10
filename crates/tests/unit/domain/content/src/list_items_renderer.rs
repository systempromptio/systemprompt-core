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

    assert!(
        html.contains(r#"<time class="card-date"></time>"#),
        "got: {html}"
    );
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

#[tokio::test]
async fn hostile_front_matter_is_html_escaped_in_text_and_attributes() {
    let html = render(&[json!({
        "content_type": "blog-list",
        "title": "<img src=x onerror=alert(1)>",
        "slug": "a\"><script>x()</script>",
        "description": "<script>alert(2)</script> & more",
        "image": "/i.png\" onerror=\"alert(3)",
    })])
    .await;

    assert!(!html.contains("<img src=x"), "got: {html}");
    assert!(!html.contains("<script>"), "got: {html}");
    assert!(!html.contains("\" onerror=\""), "got: {html}");
    assert!(
        html.contains("&lt;img src=x onerror=alert(1)&gt;"),
        "got: {html}"
    );
    assert!(html.contains("&lt;script&gt;alert(2)&lt;/script&gt; &amp; more"));
    assert!(
        html.contains(r#"src="/i.png&quot; onerror=&quot;alert(3)""#),
        "got: {html}"
    );
}

#[tokio::test]
async fn script_scheme_image_falls_back_to_the_placeholder() {
    for image in [
        "javascript:alert(1)",
        " JavaScript:alert(1)",
        "data:text/html,x",
        "vbscript:x",
    ] {
        let html = render(&[json!({
            "content_type": "blog-list", "title": "A", "slug": "a", "image": image,
        })])
        .await;
        assert!(html.contains("card-image--placeholder"), "{image}: {html}");
        assert!(
            !html.to_ascii_lowercase().contains("script:"),
            "{image}: {html}"
        );
        assert!(!html.contains("data:"), "{image}: {html}");
    }
}

#[tokio::test]
async fn slug_segments_are_percent_encoded_and_separators_kept() {
    let html = render(&[json!({
        "content_type": "blog-list", "title": "A", "slug": "docs/javascript:alert(1) x",
    })])
    .await;

    assert!(
        html.contains(r#"href="/blog/docs/javascript%3Aalert%281%29%20x""#),
        "got: {html}"
    );
}
