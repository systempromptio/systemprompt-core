//! Tests for TemplateEngine

use std::collections::HashMap;
use systemprompt_oauth::TemplateEngine;

// ============================================================================
// TemplateEngine::render Tests
// ============================================================================

#[test]
fn test_render_simple_template() {
    let template = "Hello, {name}!";
    let mut context = HashMap::new();
    context.insert("name", "World");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Hello, World!");
}

#[test]
fn test_render_multiple_placeholders() {
    let template = "{greeting}, {name}! Welcome to {place}.";
    let mut context = HashMap::new();
    context.insert("greeting", "Hello");
    context.insert("name", "Alice");
    context.insert("place", "OAuth Land");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Hello, Alice! Welcome to OAuth Land.");
}

#[test]
fn test_render_no_placeholders() {
    let template = "This is a static template.";
    let context = HashMap::new();

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "This is a static template.");
}

#[test]
fn test_render_empty_template() {
    let template = "";
    let context = HashMap::new();

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "");
}

#[test]
fn test_render_missing_placeholder() {
    let template = "Hello, {name}!";
    let context = HashMap::new();

    let result = TemplateEngine::render(template, context);
    // Placeholder should remain unchanged if not in context
    assert_eq!(result, "Hello, {name}!");
}

#[test]
fn test_render_html_escaping_ampersand() {
    let template = "Value: {value}";
    let mut context = HashMap::new();
    context.insert("value", "A & B");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: A &amp; B");
}

#[test]
fn test_render_html_escaping_less_than() {
    let template = "Value: {value}";
    let mut context = HashMap::new();
    context.insert("value", "A < B");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: A &lt; B");
}

#[test]
fn test_render_html_escaping_greater_than() {
    let template = "Value: {value}";
    let mut context = HashMap::new();
    context.insert("value", "A > B");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: A &gt; B");
}

#[test]
fn test_render_html_escaping_double_quote() {
    let template = "Value: {value}";
    let mut context = HashMap::new();
    context.insert("value", "Say \"hello\"");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: Say &quot;hello&quot;");
}

#[test]
fn test_render_html_escaping_single_quote() {
    let template = "Value: {value}";
    let mut context = HashMap::new();
    context.insert("value", "It's good");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: It&#x27;s good");
}

#[test]
fn test_render_xss_prevention() {
    let template = "Hello, {name}!";
    let mut context = HashMap::new();
    context.insert("name", "<script>alert('xss')</script>");

    let result = TemplateEngine::render(template, context);
    assert!(result.contains("&lt;script&gt;"));
    assert!(!result.contains("<script>"));
}

#[test]
fn test_render_repeated_placeholder() {
    let template = "{name} said hello to {name}!";
    let mut context = HashMap::new();
    context.insert("name", "Alice");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Alice said hello to Alice!");
}

#[test]
fn test_render_complex_html() {
    let template = "<div class=\"greeting\">{message}</div>";
    let mut context = HashMap::new();
    context.insert("message", "Hello & goodbye");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "<div class=\"greeting\">Hello &amp; goodbye</div>");
}

#[test]
fn test_render_empty_value() {
    let template = "Value: [{value}]";
    let mut context = HashMap::new();
    context.insert("value", "");

    let result = TemplateEngine::render(template, context);
    assert_eq!(result, "Value: []");
}

// ============================================================================
// TemplateEngine template loading Tests
// ============================================================================

#[test]
fn test_load_authorize_template() {
    let template = TemplateEngine::load_authorize_template();
    assert!(!template.is_empty());
    // Should be valid HTML
    assert!(template.contains("<!DOCTYPE html>") || template.contains("<html"));
}

#[test]
fn test_load_webauthn_oauth_template() {
    let template = TemplateEngine::load_webauthn_oauth_template();
    assert!(!template.is_empty());
    // Should be valid HTML
    assert!(template.contains("<!DOCTYPE html>") || template.contains("<html"));
}

// ============================================================================
// TemplateEngine struct Tests
// ============================================================================

#[test]
fn test_template_engine_copy() {
    let engine = TemplateEngine;
    let copied = engine;
    // Should compile - TemplateEngine is Copy
    let _ = copied;
    let _ = engine;
}

#[test]
fn test_template_engine_clone() {
    let engine = TemplateEngine;
    let cloned = engine.clone();
    // Should compile - TemplateEngine is Clone
    let _ = cloned;
}

#[test]
fn test_template_engine_debug() {
    let engine = TemplateEngine;
    let debug_str = format!("{:?}", engine);
    assert!(debug_str.contains("TemplateEngine"));
}
