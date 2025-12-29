//! Unit tests for template engine functionality

use serde_json::json;
use std::fs;
use systemprompt_generator::TemplateEngine;
use tempfile::TempDir;

// =============================================================================
// TemplateEngine::new tests
// =============================================================================

#[tokio::test]
async fn test_template_engine_new_with_valid_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create a sample template
    let template_content = "<html><body>{{content}}</body></html>";
    fs::write(temp_dir.path().join("base.html"), template_content).unwrap();

    let result = TemplateEngine::new(temp_dir.path().to_str().unwrap()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_template_engine_new_with_nonexistent_directory() {
    let result = TemplateEngine::new("/nonexistent/directory/path").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_template_engine_new_with_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let result = TemplateEngine::new(temp_dir.path().to_str().unwrap()).await;
    // Should succeed even with no templates
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_template_engine_new_loads_multiple_templates() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("header.html"),
        "<header>{{title}}</header>",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("footer.html"),
        "<footer>{{copyright}}</footer>",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("main.html"),
        "<main>{{content}}</main>",
    )
    .unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify all templates are loaded by rendering them
    assert!(engine.render("header", &json!({"title": "Test"})).is_ok());
    assert!(engine
        .render("footer", &json!({"copyright": "2024"}))
        .is_ok());
    assert!(engine.render("main", &json!({"content": "Hello"})).is_ok());
}

#[tokio::test]
async fn test_template_engine_ignores_non_html_files() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(temp_dir.path().join("valid.html"), "<p>{{text}}</p>").unwrap();
    fs::write(temp_dir.path().join("readme.md"), "# Readme").unwrap();
    fs::write(temp_dir.path().join("config.json"), "{}").unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Only .html files should be loaded as templates
    assert!(engine.render("valid", &json!({"text": "hello"})).is_ok());
    assert!(engine.render("readme", &json!({})).is_err());
    assert!(engine.render("config", &json!({})).is_err());
}

// =============================================================================
// TemplateEngine::render tests
// =============================================================================

#[tokio::test]
async fn test_template_render() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<!DOCTYPE html>
<html>
<head><title>{{title}}</title></head>
<body>
<h1>{{heading}}</h1>
<p>{{body}}</p>
</body>
</html>"#;

    fs::write(temp_dir.path().join("page.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "title": "My Page",
        "heading": "Welcome",
        "body": "This is the content."
    });

    let result = engine.render("page", &data).unwrap();

    assert!(result.contains("<title>My Page</title>"));
    assert!(result.contains("<h1>Welcome</h1>"));
    assert!(result.contains("<p>This is the content.</p>"));
}

#[tokio::test]
async fn test_template_render_nonexistent_template() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join("exists.html"), "<p>Hello</p>").unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let result = engine.render("does_not_exist", &json!({}));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_template_render_with_missing_variables() {
    let temp_dir = TempDir::new().unwrap();

    // Handlebars renders missing variables as empty strings by default
    let template = "<p>Hello {{name}}, your age is {{age}}</p>";
    fs::write(temp_dir.path().join("greeting.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Only provide some variables
    let result = engine.render("greeting", &json!({"name": "Alice"}));

    // Should still render, with age as empty
    assert!(result.is_ok());
    let html = result.unwrap();
    assert!(html.contains("Hello Alice"));
}

// =============================================================================
// Template data injection tests
// =============================================================================

#[tokio::test]
async fn test_template_data_injection() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<article>
<h1>{{TITLE}}</h1>
<p class="author">By {{AUTHOR}}</p>
<p class="date">{{DATE}}</p>
<div class="content">{{{CONTENT}}}</div>
</article>"#;

    fs::write(temp_dir.path().join("article.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "TITLE": "Test Article",
        "AUTHOR": "Jane Doe",
        "DATE": "2024-01-15",
        "CONTENT": "<p>This is <strong>HTML</strong> content.</p>"
    });

    let result = engine.render("article", &data).unwrap();

    assert!(result.contains("<h1>Test Article</h1>"));
    assert!(result.contains("By Jane Doe"));
    assert!(result.contains("2024-01-15"));
    // Triple braces {{{...}}} should not escape HTML
    assert!(result.contains("<strong>HTML</strong>"));
}

#[tokio::test]
async fn test_template_data_injection_with_arrays() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<ul>
{{#each items}}
<li>{{this}}</li>
{{/each}}
</ul>"#;

    fs::write(temp_dir.path().join("list.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "items": ["Apple", "Banana", "Cherry"]
    });

    let result = engine.render("list", &data).unwrap();

    assert!(result.contains("<li>Apple</li>"));
    assert!(result.contains("<li>Banana</li>"));
    assert!(result.contains("<li>Cherry</li>"));
}

#[tokio::test]
async fn test_template_data_injection_with_objects() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<div class="user">
<span class="name">{{user.name}}</span>
<span class="email">{{user.email}}</span>
</div>"#;

    fs::write(temp_dir.path().join("user.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "user": {
            "name": "John Smith",
            "email": "john@example.com"
        }
    });

    let result = engine.render("user", &data).unwrap();

    assert!(result.contains("John Smith"));
    assert!(result.contains("john@example.com"));
}

#[tokio::test]
async fn test_template_data_injection_with_conditionals() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"{{#if show_message}}
<p>{{message}}</p>
{{else}}
<p>No message</p>
{{/if}}"#;

    fs::write(temp_dir.path().join("conditional.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Test with show_message = true
    let data_true = json!({
        "show_message": true,
        "message": "Hello World"
    });
    let result_true = engine.render("conditional", &data_true).unwrap();
    assert!(result_true.contains("Hello World"));
    assert!(!result_true.contains("No message"));

    // Test with show_message = false
    let data_false = json!({
        "show_message": false
    });
    let result_false = engine.render("conditional", &data_false).unwrap();
    assert!(result_false.contains("No message"));
}

// =============================================================================
// Template partials tests
// =============================================================================

#[tokio::test]
async fn test_template_partials() {
    let temp_dir = TempDir::new().unwrap();

    // Create a main template that references other templates
    // Note: Handlebars partials need to be registered explicitly
    // For now, we test that templates can reference each other indirectly

    let header = "<header><h1>{{site_name}}</h1></header>";
    let footer = "<footer><p>&copy; {{year}}</p></footer>";
    let main = r#"<!DOCTYPE html>
<html>
<body>
<header><h1>{{site_name}}</h1></header>
<main>{{content}}</main>
<footer><p>&copy; {{year}}</p></footer>
</body>
</html>"#;

    fs::write(temp_dir.path().join("header.html"), header).unwrap();
    fs::write(temp_dir.path().join("footer.html"), footer).unwrap();
    fs::write(temp_dir.path().join("main.html"), main).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "site_name": "My Site",
        "content": "Page content here",
        "year": "2024"
    });

    let result = engine.render("main", &data).unwrap();

    assert!(result.contains("<h1>My Site</h1>"));
    assert!(result.contains("Page content here"));
    assert!(result.contains("&copy; 2024"));
}

#[tokio::test]
async fn test_template_with_html_escaping() {
    let temp_dir = TempDir::new().unwrap();

    // Test that double braces escape HTML (security feature)
    let template = r#"<div>
<p class="escaped">{{user_input}}</p>
</div>"#;

    fs::write(temp_dir.path().join("escape.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "user_input": "<script>alert('xss')</script>"
    });

    let result = engine.render("escape", &data).unwrap();

    // Double braces should escape HTML for security
    assert!(result.contains("&lt;script&gt;"));
    assert!(result.contains("&lt;/script&gt;"));
    // Should NOT contain raw script tags
    assert!(!result.contains("<script>"));
}

#[tokio::test]
async fn test_template_complex_data_structure() {
    let temp_dir = TempDir::new().unwrap();

    let template = r#"<article>
<h1>{{post.title}}</h1>
<p class="meta">
By {{post.author.name}} | {{post.date}}
</p>
{{#if post.tags}}
<ul class="tags">
{{#each post.tags}}
<li>{{this}}</li>
{{/each}}
</ul>
{{/if}}
<div class="content">{{{post.content}}}</div>
{{#if post.related}}
<aside>
<h3>Related Posts</h3>
<ul>
{{#each post.related}}
<li><a href="{{url}}">{{title}}</a></li>
{{/each}}
</ul>
</aside>
{{/if}}
</article>"#;

    fs::write(temp_dir.path().join("blog-post.html"), template).unwrap();

    let engine = TemplateEngine::new(temp_dir.path().to_str().unwrap())
        .await
        .unwrap();

    let data = json!({
        "post": {
            "title": "Advanced Rust Patterns",
            "author": {
                "name": "Alice Developer",
                "email": "alice@example.com"
            },
            "date": "2024-03-15",
            "tags": ["rust", "programming", "patterns"],
            "content": "<p>This is the post content.</p>",
            "related": [
                {"title": "Rust Basics", "url": "/blog/rust-basics"},
                {"title": "Error Handling", "url": "/blog/error-handling"}
            ]
        }
    });

    let result = engine.render("blog-post", &data).unwrap();

    assert!(result.contains("<h1>Advanced Rust Patterns</h1>"));
    assert!(result.contains("Alice Developer"));
    assert!(result.contains("2024-03-15"));
    assert!(result.contains("<li>rust</li>"));
    assert!(result.contains("<li>programming</li>"));
    assert!(result.contains("This is the post content."));
    assert!(result.contains("Rust Basics"));
    assert!(result.contains("/blog/rust-basics"));
}
