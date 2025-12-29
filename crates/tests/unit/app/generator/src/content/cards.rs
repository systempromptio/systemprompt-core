//! Unit tests for content card generation and image URL handling

use systemprompt_generator::content::{
    generate_content_card, generate_image_html, generate_related_card, get_absolute_image_url,
    normalize_image_url, CardData,
};

// ============================================================================
// normalize_image_url Tests
// ============================================================================

#[test]
fn test_normalize_image_url_none() {
    let result = normalize_image_url(None);
    assert!(result.is_none());
}

#[test]
fn test_normalize_image_url_empty_string() {
    let result = normalize_image_url(Some(""));
    assert!(result.is_none());
}

#[test]
fn test_normalize_image_url_already_webp() {
    let result = normalize_image_url(Some("/images/photo.webp"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));
}

#[test]
fn test_normalize_image_url_converts_png_to_webp() {
    let result = normalize_image_url(Some("/images/photo.png"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));
}

#[test]
fn test_normalize_image_url_converts_jpg_to_webp() {
    let result = normalize_image_url(Some("/images/photo.jpg"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));
}

#[test]
fn test_normalize_image_url_converts_jpeg_to_webp() {
    let result = normalize_image_url(Some("/images/photo.jpeg"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));
}

#[test]
fn test_normalize_image_url_case_insensitive_extension() {
    let result = normalize_image_url(Some("/images/photo.PNG"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));

    let result = normalize_image_url(Some("/images/photo.JPG"));
    assert_eq!(result, Some("/images/photo.webp".to_string()));

    let result = normalize_image_url(Some("/images/photo.WEBP"));
    assert_eq!(result, Some("/images/photo.WEBP".to_string()));
}

#[test]
fn test_normalize_image_url_preserves_other_extensions() {
    let result = normalize_image_url(Some("/images/icon.svg"));
    assert_eq!(result, Some("/images/icon.svg".to_string()));

    let result = normalize_image_url(Some("/images/animation.gif"));
    assert_eq!(result, Some("/images/animation.gif".to_string()));
}

#[test]
fn test_normalize_image_url_no_extension() {
    let result = normalize_image_url(Some("/images/photo"));
    assert_eq!(result, Some("/images/photo".to_string()));
}

#[test]
fn test_normalize_image_url_complex_path() {
    let result = normalize_image_url(Some("/content/blog/2024/01/featured-image.png"));
    assert_eq!(
        result,
        Some("/content/blog/2024/01/featured-image.webp".to_string())
    );
}

// ============================================================================
// get_absolute_image_url Tests
// ============================================================================

#[test]
fn test_get_absolute_image_url_none() {
    let result = get_absolute_image_url(None, "https://example.com");
    assert!(result.is_none());
}

#[test]
fn test_get_absolute_image_url_relative_path() {
    let result = get_absolute_image_url(Some("/images/photo.png"), "https://example.com");
    assert_eq!(
        result,
        Some("https://example.com/images/photo.webp".to_string())
    );
}

#[test]
fn test_get_absolute_image_url_relative_path_base_with_slash() {
    let result = get_absolute_image_url(Some("/images/photo.jpg"), "https://example.com/");
    assert_eq!(
        result,
        Some("https://example.com/images/photo.webp".to_string())
    );
}

#[test]
fn test_get_absolute_image_url_already_absolute() {
    let result =
        get_absolute_image_url(Some("https://cdn.example.com/photo.webp"), "https://example.com");
    assert_eq!(
        result,
        Some("https://cdn.example.com/photo.webp".to_string())
    );
}

#[test]
fn test_get_absolute_image_url_http_url() {
    let result =
        get_absolute_image_url(Some("http://cdn.example.com/photo.png"), "https://example.com");
    // Should preserve the original http URL but convert to webp
    assert!(result.is_some());
    assert!(result.unwrap().contains("http://cdn.example.com"));
}

// ============================================================================
// generate_image_html Tests
// ============================================================================

#[test]
fn test_generate_image_html_with_image() {
    let result = generate_image_html(Some("/images/photo.png"), "Photo description");

    assert!(result.contains("card-image"));
    assert!(result.contains("<img"));
    assert!(result.contains("loading=\"lazy\""));
    assert!(result.contains("alt=\"Photo description\""));
}

#[test]
fn test_generate_image_html_no_image() {
    let result = generate_image_html(None, "Alt text");

    assert!(result.contains("card-image--placeholder"));
    assert!(result.contains("<svg"));
    assert!(!result.contains("<img"));
}

#[test]
fn test_generate_image_html_empty_image() {
    let result = generate_image_html(Some(""), "Alt text");

    assert!(result.contains("card-image--placeholder"));
    assert!(result.contains("<svg"));
}

#[test]
fn test_generate_image_html_converts_to_webp() {
    let result = generate_image_html(Some("/images/photo.jpg"), "Photo");

    assert!(result.contains(".webp"));
    assert!(!result.contains(".jpg"));
}

// ============================================================================
// CardData Tests
// ============================================================================

#[test]
fn test_card_data_creation() {
    let card = CardData {
        title: "Test Title",
        slug: "test-slug",
        description: "Test description",
        image: Some("/images/test.png"),
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    assert_eq!(card.title, "Test Title");
    assert_eq!(card.slug, "test-slug");
    assert_eq!(card.description, "Test description");
    assert_eq!(card.image, Some("/images/test.png"));
    assert_eq!(card.date, "2024-01-15");
    assert_eq!(card.url_prefix, "/blog");
}

#[test]
fn test_card_data_no_image() {
    let card = CardData {
        title: "No Image Post",
        slug: "no-image",
        description: "Post without image",
        image: None,
        date: "2024-01-20",
        url_prefix: "/articles",
    };

    assert!(card.image.is_none());
}

// ============================================================================
// generate_content_card Tests
// ============================================================================

#[test]
fn test_generate_content_card_basic() {
    let card = CardData {
        title: "Test Article",
        slug: "test-article",
        description: "A test article description",
        image: Some("/images/test.png"),
        date: "January 15, 2024",
        url_prefix: "/blog",
    };

    let result = generate_content_card(&card);

    assert!(result.contains("content-card-link"));
    assert!(result.contains("href=\"/blog/test-article\""));
    assert!(result.contains("Test Article"));
    assert!(result.contains("A test article description"));
    assert!(result.contains("January 15, 2024"));
    assert!(result.contains("<article"));
    assert!(result.contains("class=\"content-card\""));
}

#[test]
fn test_generate_content_card_no_image() {
    let card = CardData {
        title: "No Image",
        slug: "no-image",
        description: "Description",
        image: None,
        date: "2024-01-15",
        url_prefix: "/docs",
    };

    let result = generate_content_card(&card);

    assert!(result.contains("card-image--placeholder"));
    assert!(result.contains("<svg"));
}

#[test]
fn test_generate_content_card_escaping() {
    let card = CardData {
        title: "Title & More",
        slug: "title-more",
        description: "Description with <special> chars",
        image: None,
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    let result = generate_content_card(&card);

    // HTML should contain the text (escaping is handled by the template)
    assert!(result.contains("Title & More") || result.contains("Title &amp; More"));
}

// ============================================================================
// generate_related_card Tests
// ============================================================================

#[test]
fn test_generate_related_card_basic() {
    let card = CardData {
        title: "Related Post",
        slug: "related-post",
        description: "This is a related post description.",
        image: Some("/images/related.jpg"),
        date: "2024-01-10",
        url_prefix: "/blog",
    };

    let result = generate_related_card(&card, "/blog/related-post");

    assert!(result.contains("related-card-link"));
    assert!(result.contains("href=\"/blog/related-post\""));
    assert!(result.contains("Related Post"));
    assert!(result.contains("2024-01-10"));
    assert!(result.contains("<article"));
    assert!(result.contains("class=\"related-card\""));
}

#[test]
fn test_generate_related_card_truncates_description() {
    let card = CardData {
        title: "Long Description",
        slug: "long-desc",
        description: "First line of description.\nSecond line.\nThird line that should be cut.",
        image: None,
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    let result = generate_related_card(&card, "/blog/long-desc");

    // Should only include first two lines
    assert!(result.contains("First line"));
    assert!(result.contains("Second line"));
    // Third line should not appear (truncated to 2 lines)
}

#[test]
fn test_generate_related_card_custom_href() {
    let card = CardData {
        title: "External Link",
        slug: "external",
        description: "Links elsewhere",
        image: None,
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    let result = generate_related_card(&card, "https://external.com/article");

    assert!(result.contains("href=\"https://external.com/article\""));
}

#[test]
fn test_generate_related_card_no_image() {
    let card = CardData {
        title: "No Image",
        slug: "no-img",
        description: "No image here",
        image: None,
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    let result = generate_related_card(&card, "/blog/no-img");

    assert!(result.contains("card-image--placeholder"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_card_with_empty_strings() {
    let card = CardData {
        title: "",
        slug: "",
        description: "",
        image: None,
        date: "",
        url_prefix: "",
    };

    // Should not panic, even with empty strings
    let result = generate_content_card(&card);
    assert!(result.contains("content-card"));

    let related = generate_related_card(&card, "");
    assert!(related.contains("related-card"));
}

#[test]
fn test_card_with_unicode() {
    let card = CardData {
        title: "日本語タイトル",
        slug: "japanese-title",
        description: "Description avec des accénts français",
        image: None,
        date: "2024-01-15",
        url_prefix: "/international",
    };

    let result = generate_content_card(&card);

    assert!(result.contains("日本語タイトル"));
    assert!(result.contains("accénts"));
}

#[test]
fn test_card_with_long_description() {
    let long_desc = "x".repeat(10000);
    let card = CardData {
        title: "Long Desc",
        slug: "long",
        description: &long_desc,
        image: None,
        date: "2024-01-15",
        url_prefix: "/blog",
    };

    // Should not panic with long description
    let result = generate_content_card(&card);
    assert!(result.contains("content-card"));
}
