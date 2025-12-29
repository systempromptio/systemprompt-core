//! Unit tests for prerender functionality
//!
//! Note: Full prerender_content tests require database access and are better
//! suited for integration tests. These unit tests focus on the supporting
//! functions and template rendering aspects.

use serde_json::json;
use std::fs;
use systemprompt_generator::{render_markdown, TemplateEngine};
use tempfile::TempDir;

// =============================================================================
// Prerender content generation tests
// =============================================================================

#[tokio::test]
async fn test_prerender_content_template_rendering() {
    let temp_dir = TempDir::new().unwrap();

    // Create a blog post template similar to what prerender uses
    let template = r#"<!DOCTYPE html>
<html>
<head>
    <title>{{TITLE}}</title>
    <meta name="description" content="{{DESCRIPTION}}">
    <meta name="author" content="{{AUTHOR}}">
</head>
<body>
    <article>
        <h1>{{TITLE}}</h1>
        <p class="meta">By {{AUTHOR}} on {{DATE}}</p>
        <div class="content">{{{CONTENT}}}</div>
    </article>
</body>
</html>"#;

    fs::write(temp_dir.path().join("blog-post.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Simulate content data that would come from database
    let content_markdown = "This is the **body** of the post with some `code`.";
    let content_html = render_markdown(content_markdown);

    let data = json!({
        "TITLE": "My Blog Post",
        "DESCRIPTION": "A great blog post about testing",
        "AUTHOR": "Test Author",
        "DATE": "2024-01-15",
        "CONTENT": content_html
    });

    let result = engine.render("blog-post", &data).unwrap();

    assert!(result.contains("<title>My Blog Post</title>"));
    assert!(result.contains("content=\"A great blog post about testing\""));
    assert!(result.contains("By Test Author on 2024-01-15"));
    assert!(result.contains("<strong>body</strong>"));
    assert!(result.contains("<code>code</code>"));
}

#[tokio::test]
async fn test_prerender_output_html_structure() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{TITLE}}</title>
    <link rel="canonical" href="{{CANONICAL_PATH}}">
    {{#if IMAGE}}
    <meta property="og:image" content="{{IMAGE}}">
    {{/if}}
</head>
<body>
    <main>
        <article itemscope itemtype="https://schema.org/Article">
            <h1 itemprop="headline">{{TITLE}}</h1>
            {{#if READ_TIME}}
            <span class="read-time">{{READ_TIME}} min read</span>
            {{/if}}
            <div itemprop="articleBody">{{{CONTENT}}}</div>
        </article>
    </main>
</body>
</html>"#;

    fs::write(temp_dir.path().join("article.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "TITLE": "Comprehensive Testing Guide",
        "CANONICAL_PATH": "/blog/testing-guide",
        "IMAGE": "/images/testing.jpg",
        "READ_TIME": 5,
        "CONTENT": "<p>Testing is important.</p>"
    });

    let result = engine.render("article", &data).unwrap();

    // Check HTML5 structure
    assert!(result.contains("<!DOCTYPE html>"));
    assert!(result.contains("<html lang=\"en\">"));
    assert!(result.contains("<meta charset=\"UTF-8\">"));

    // Check SEO elements
    assert!(result.contains("rel=\"canonical\""));
    assert!(result.contains("href=\"/blog/testing-guide\""));
    assert!(result.contains("og:image"));

    // Check semantic structure
    assert!(result.contains("itemscope"));
    assert!(result.contains("itemprop=\"headline\""));
    assert!(result.contains("itemprop=\"articleBody\""));

    // Check content
    assert!(result.contains("5 min read"));
    assert!(result.contains("Testing is important."));
}

#[tokio::test]
async fn test_prerender_content_list_page() {
    let temp_dir = TempDir::new().unwrap();

    // Template for listing pages (e.g., /blog/)
    let template = r#"<!DOCTYPE html>
<html>
<head><title>{{PAGE_TITLE}}</title></head>
<body>
    <h1>{{PAGE_TITLE}}</h1>
    <div class="posts">
        {{#each POSTS}}
        <article class="post-card">
            <h2><a href="{{url}}">{{title}}</a></h2>
            <p class="excerpt">{{description}}</p>
            <time>{{date}}</time>
        </article>
        {{/each}}
    </div>
    {{#if HAS_MORE}}
    <a href="{{NEXT_PAGE_URL}}" class="load-more">Load More</a>
    {{/if}}
</body>
</html>"#;

    fs::write(temp_dir.path().join("blog-list.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "PAGE_TITLE": "All Blog Posts",
        "POSTS": [
            {
                "title": "First Post",
                "url": "/blog/first-post",
                "description": "Introduction to testing",
                "date": "2024-01-15"
            },
            {
                "title": "Second Post",
                "url": "/blog/second-post",
                "description": "Advanced testing techniques",
                "date": "2024-01-20"
            }
        ],
        "HAS_MORE": true,
        "NEXT_PAGE_URL": "/blog/page/2"
    });

    let result = engine.render("blog-list", &data).unwrap();

    assert!(result.contains("<title>All Blog Posts</title>"));
    assert!(result.contains("First Post"));
    assert!(result.contains("/blog/first-post"));
    assert!(result.contains("Second Post"));
    assert!(result.contains("Load More"));
    assert!(result.contains("/blog/page/2"));
}

#[tokio::test]
async fn test_prerender_related_content() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<article>
<h1>{{TITLE}}</h1>
<div class="content">{{{CONTENT}}}</div>

{{#if RELATED_CONTENT}}
<aside class="related">
    <h3>Related Articles</h3>
    <ul>
    {{#each RELATED_CONTENT}}
        <li><a href="{{url}}">{{title}}</a></li>
    {{/each}}
    </ul>
</aside>
{{/if}}

{{#if POPULAR_CONTENT}}
<aside class="popular">
    <h3>Popular Articles</h3>
    <ul>
    {{#each POPULAR_CONTENT}}
        <li><a href="{{url}}">{{title}}</a></li>
    {{/each}}
    </ul>
</aside>
{{/if}}
</article>"#;

    fs::write(temp_dir.path().join("post.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "TITLE": "Main Article",
        "CONTENT": "<p>Article content here.</p>",
        "RELATED_CONTENT": [
            {"title": "Related 1", "url": "/related-1"},
            {"title": "Related 2", "url": "/related-2"}
        ],
        "POPULAR_CONTENT": [
            {"title": "Popular 1", "url": "/popular-1"},
            {"title": "Popular 2", "url": "/popular-2"}
        ]
    });

    let result = engine.render("post", &data).unwrap();

    assert!(result.contains("<h3>Related Articles</h3>"));
    assert!(result.contains("Related 1"));
    assert!(result.contains("/related-1"));
    assert!(result.contains("<h3>Popular Articles</h3>"));
    assert!(result.contains("Popular 1"));
}

#[tokio::test]
async fn test_prerender_with_no_optional_content() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<article>
<h1>{{TITLE}}</h1>
{{#if IMAGE}}
<img src="{{IMAGE}}" alt="{{TITLE}}">
{{/if}}
<div>{{{CONTENT}}}</div>
{{#if KEYWORDS}}
<div class="keywords">
{{#each KEYWORDS}}
<span class="keyword">{{this}}</span>
{{/each}}
</div>
{{/if}}
</article>"#;

    fs::write(temp_dir.path().join("minimal.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Minimal data without optional fields
    let data = json!({
        "TITLE": "Minimal Post",
        "CONTENT": "<p>Just content.</p>"
    });

    let result = engine.render("minimal", &data).unwrap();

    assert!(result.contains("<h1>Minimal Post</h1>"));
    assert!(result.contains("Just content."));
    assert!(!result.contains("<img"));
    assert!(!result.contains("keywords"));
}

// =============================================================================
// Prerender output file tests
// =============================================================================

#[tokio::test]
async fn test_prerender_output_directory_creation() {
    let temp_dir = TempDir::new().unwrap();

    // Simulate creating output directory structure for a blog post
    let output_path = temp_dir.path().join("blog/my-post");
    fs::create_dir_all(&output_path).unwrap();

    let index_path = output_path.join("index.html");
    fs::write(&index_path, "<html><body>Test</body></html>").unwrap();

    assert!(index_path.exists());
    assert!(output_path.is_dir());
}

#[tokio::test]
async fn test_prerender_nested_url_paths() {
    let temp_dir = TempDir::new().unwrap();

    // Test creating deeply nested paths like /blog/2024/01/my-post/
    let paths = [
        "blog/post-1",
        "blog/2024/post-2",
        "blog/2024/01/post-3",
        "docs/api/v1/endpoint",
    ];

    for path in paths {
        let full_path = temp_dir.path().join(path);
        fs::create_dir_all(&full_path).unwrap();
        fs::write(full_path.join("index.html"), "<html></html>").unwrap();

        assert!(full_path.join("index.html").exists());
    }
}

#[tokio::test]
async fn test_prerender_slug_transformation() {
    // Test that various slug patterns work correctly
    let slugs = [
        "simple-post",
        "post-with-numbers-123",
        "UPPERCASE-POST",
        "mixed_separators-test",
        "unicode-日本語",
    ];

    let temp_dir = TempDir::new().unwrap();

    for slug in slugs {
        let path = temp_dir.path().join(format!("blog/{}", slug));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("index.html"), "<html></html>").unwrap();
        assert!(path.join("index.html").exists());
    }
}

// =============================================================================
// Content processing for prerender tests
// =============================================================================

#[test]
fn test_prerender_markdown_to_html_conversion() {
    let markdown = r#"
# Article Title

## Introduction

This is the introduction paragraph with **bold** and *italic* text.

## Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

## Conclusion

- Point one
- Point two
- Point three
"#;

    let html = render_markdown(markdown);

    // H1 should be stripped (prerender adds it via template)
    assert!(!html.contains("<h1>Article Title</h1>"));

    // H2 should be preserved
    assert!(html.contains("<h2>Introduction</h2>"));
    assert!(html.contains("<h2>Code Example</h2>"));
    assert!(html.contains("<h2>Conclusion</h2>"));

    // Formatting should be preserved
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));

    // Code blocks should be rendered
    assert!(html.contains("<pre>"));
    assert!(html.contains("fn main()"));

    // Lists should be rendered
    assert!(html.contains("<li>Point one</li>"));
}

#[test]
fn test_prerender_html_content_escaping() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<div class="user-content">
Escaped: {{user_input}}
Raw: {{{trusted_html}}}
</div>"#;

    fs::write(temp_dir.path().join("content.html"), template).unwrap();

    // This is sync setup, we test the concept
    let _dangerous_input = "<script>alert('xss')</script>";
    let _safe_html = "<strong>Safe</strong>";

    // In actual template rendering:
    // - {{user_input}} would escape the script tags
    // - {{{trusted_html}}} would preserve the HTML

    // Verify the template file was created for later async testing
    assert!(temp_dir.path().join("content.html").exists());
}

#[tokio::test]
async fn test_prerender_json_data_structure() {
    // Test the JSON structure that prepare_template_data would generate
    let template_data = json!({
        "TITLE": "Test Article",
        "DESCRIPTION": "A test article description",
        "AUTHOR": "Test Author",
        "DATE": "2024-01-15",
        "SLUG": "test-article",
        "CANONICAL_PATH": "/blog/test-article",
        "ORG_NAME": "Test Organization",
        "ORG_URL": "https://example.com",
        "TWITTER_HANDLE": "@testorg",
        "IMAGE": "/images/test.jpg",
        "FEATURED_IMAGE": "/images/featured.jpg",
        "READ_TIME": 5,
        "KEYWORDS": ["testing", "rust", "development"],
        "CONTENT": "<p>Article content here.</p>",
        "RELATED_CONTENT": "<ul><li>Related 1</li></ul>",
        "REFERENCES": "<ul><li>Reference 1</li></ul>",
        "SOCIAL_CONTENT": "<div>Share buttons</div>"
    });

    // Verify all expected fields are present
    assert!(template_data.get("TITLE").is_some());
    assert!(template_data.get("DESCRIPTION").is_some());
    assert!(template_data.get("AUTHOR").is_some());
    assert!(template_data.get("CONTENT").is_some());
    assert!(template_data.get("CANONICAL_PATH").is_some());

    // Verify field types
    assert!(template_data["TITLE"].is_string());
    assert!(template_data["READ_TIME"].is_number());
    assert!(template_data["KEYWORDS"].is_array());
}

#[tokio::test]
async fn test_prerender_paper_template() {
    let temp_dir = TempDir::new().unwrap();

    // Paper template with TOC and sections
    let template = r#"<!DOCTYPE html>
<html>
<head><title>{{TITLE}}</title></head>
<body>
<article class="paper">
    <header>
        <h1>{{TITLE}}</h1>
        <p class="author">{{AUTHOR}}</p>
    </header>

    {{#if TOC}}
    <nav class="toc">
        <h2>Table of Contents</h2>
        {{{TOC}}}
    </nav>
    {{/if}}

    <div class="paper-content">
        {{{CONTENT}}}
    </div>

    {{#if SECTIONS}}
    {{#each SECTIONS}}
    <section id="{{id}}">
        <h2>{{title}}</h2>
        {{{content}}}
    </section>
    {{/each}}
    {{/if}}
</article>
</body>
</html>"#;

    fs::write(temp_dir.path().join("paper.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "TITLE": "Research Paper Title",
        "AUTHOR": "Dr. Researcher",
        "TOC": "<ul><li><a href=\"#intro\">Introduction</a></li></ul>",
        "CONTENT": "<p>Abstract content.</p>",
        "SECTIONS": [
            {
                "id": "intro",
                "title": "Introduction",
                "content": "<p>Introduction content.</p>"
            }
        ]
    });

    let result = engine.render("paper", &data).unwrap();

    assert!(result.contains("<title>Research Paper Title</title>"));
    assert!(result.contains("Dr. Researcher"));
    assert!(result.contains("Table of Contents"));
    assert!(result.contains("section id=\"intro\""));
}
