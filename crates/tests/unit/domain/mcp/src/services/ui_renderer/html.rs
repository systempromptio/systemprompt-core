use systemprompt_mcp::services::ui_renderer::templates::html::{
    HtmlBuilder, html_escape, json_to_js_literal,
};

#[test]
fn html_escape_ampersand() {
    assert_eq!(html_escape("a&b"), "a&amp;b");
}

#[test]
fn html_escape_less_than() {
    assert_eq!(html_escape("a<b"), "a&lt;b");
}

#[test]
fn html_escape_greater_than() {
    assert_eq!(html_escape("a>b"), "a&gt;b");
}

#[test]
fn html_escape_double_quote() {
    assert_eq!(html_escape(r#"a"b"#), "a&quot;b");
}

#[test]
fn html_escape_single_quote() {
    assert_eq!(html_escape("a'b"), "a&#39;b");
}

#[test]
fn html_escape_empty_string() {
    assert_eq!(html_escape(""), "");
}

#[test]
fn html_escape_no_special_chars() {
    assert_eq!(html_escape("hello world"), "hello world");
}

#[test]
fn html_escape_all_special_chars_combined() {
    assert_eq!(
        html_escape(r#"<script>alert("xss")&'</script>"#),
        "&lt;script&gt;alert(&quot;xss&quot;)&amp;&#39;&lt;/script&gt;"
    );
}

#[test]
fn html_escape_multiple_ampersands() {
    assert_eq!(html_escape("a&&b"), "a&amp;&amp;b");
}

#[test]
fn html_escape_html_tag_injection() {
    assert_eq!(
        html_escape("<img src=x onerror=alert(1)>"),
        "&lt;img src=x onerror=alert(1)&gt;"
    );
}

#[test]
fn json_to_js_literal_null() {
    let value = serde_json::Value::Null;
    assert_eq!(json_to_js_literal(&value), "null");
}

#[test]
fn json_to_js_literal_boolean_true() {
    let value = serde_json::json!(true);
    assert_eq!(json_to_js_literal(&value), "true");
}

#[test]
fn json_to_js_literal_boolean_false() {
    let value = serde_json::json!(false);
    assert_eq!(json_to_js_literal(&value), "false");
}

#[test]
fn json_to_js_literal_integer() {
    let value = serde_json::json!(42);
    assert_eq!(json_to_js_literal(&value), "42");
}

#[test]
fn json_to_js_literal_float() {
    let value = serde_json::json!(3.14);
    assert_eq!(json_to_js_literal(&value), "3.14");
}

#[test]
fn json_to_js_literal_string() {
    let value = serde_json::json!("hello");
    assert_eq!(json_to_js_literal(&value), r#""hello""#);
}

#[test]
fn json_to_js_literal_empty_array() {
    let value = serde_json::json!([]);
    assert_eq!(json_to_js_literal(&value), "[]");
}

#[test]
fn json_to_js_literal_array_with_values() {
    let value = serde_json::json!([1, 2, 3]);
    assert_eq!(json_to_js_literal(&value), "[1,2,3]");
}

#[test]
fn json_to_js_literal_empty_object() {
    let value = serde_json::json!({});
    assert_eq!(json_to_js_literal(&value), "{}");
}

#[test]
fn json_to_js_literal_object_with_values() {
    let value = serde_json::json!({"key": "value"});
    assert_eq!(json_to_js_literal(&value), r#"{"key":"value"}"#);
}

#[test]
fn json_to_js_literal_nested_object() {
    let value = serde_json::json!({"a": {"b": 1}});
    let result = json_to_js_literal(&value);
    assert!(result.contains(r#""a""#));
    assert!(result.contains(r#""b""#));
}

#[test]
fn json_to_js_literal_string_with_special_chars() {
    let value = serde_json::json!("line1\nline2");
    let result = json_to_js_literal(&value);
    assert!(result.contains("\\n"));
}

#[test]
fn html_builder_minimal() {
    let html = HtmlBuilder::new("Test").build();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<title>Test</title>"));
    assert!(html.contains("<html lang=\"en\">"));
    assert!(html.contains("</html>"));
}

#[test]
fn html_builder_title_escaped() {
    let html = HtmlBuilder::new("<script>alert(1)</script>").build();
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
}

#[test]
fn html_builder_with_single_style() {
    let html = HtmlBuilder::new("Test")
        .add_style("body { color: red; }")
        .build();
    assert!(html.contains("<style>"));
    assert!(html.contains("body { color: red; }"));
    assert!(html.contains("</style>"));
}

#[test]
fn html_builder_with_multiple_styles() {
    let html = HtmlBuilder::new("Test")
        .add_style("body { color: red; }")
        .add_style("h1 { font-size: 24px; }")
        .build();
    assert!(html.contains("body { color: red; }"));
    assert!(html.contains("h1 { font-size: 24px; }"));
}

#[test]
fn html_builder_no_styles_no_style_tag() {
    let html = HtmlBuilder::new("Test").build();
    assert!(!html.contains("<style>"));
}

#[test]
fn html_builder_with_script() {
    let html = HtmlBuilder::new("Test")
        .add_script("console.log('hello');")
        .build();
    assert!(html.contains("<script>"));
    assert!(html.contains("console.log('hello');"));
    assert!(html.contains("</script>"));
}

#[test]
fn html_builder_with_multiple_scripts() {
    let html = HtmlBuilder::new("Test")
        .add_script("var a = 1;")
        .add_script("var b = 2;")
        .build();
    assert!(html.contains("var a = 1;"));
    assert!(html.contains("var b = 2;"));
}

#[test]
fn html_builder_no_scripts_no_script_tag() {
    let html = HtmlBuilder::new("Test").build();
    assert!(!html.contains("<script>"));
}

#[test]
fn html_builder_with_body() {
    let html = HtmlBuilder::new("Test")
        .body("<div>Content</div>")
        .build();
    assert!(html.contains("<div>Content</div>"));
}

#[test]
fn html_builder_body_placed_between_body_tags() {
    let html = HtmlBuilder::new("Test")
        .body("<p>Hello</p>")
        .build();
    assert!(html.contains("<body>\n<p>Hello</p>"));
}

#[test]
fn html_builder_has_meta_charset() {
    let html = HtmlBuilder::new("Test").build();
    assert!(html.contains(r#"<meta charset="UTF-8">"#));
}

#[test]
fn html_builder_has_viewport_meta() {
    let html = HtmlBuilder::new("Test").build();
    assert!(html.contains("viewport"));
    assert!(html.contains("width=device-width"));
}

#[test]
fn html_builder_full_page() {
    let html = HtmlBuilder::new("Full Page")
        .add_style("body { margin: 0; }")
        .body("<main>Content</main>")
        .add_script("init();")
        .build();

    assert!(html.contains("<title>Full Page</title>"));
    assert!(html.contains("body { margin: 0; }"));
    assert!(html.contains("<main>Content</main>"));
    assert!(html.contains("init();"));
}

#[test]
fn html_builder_styles_in_head_scripts_in_body() {
    let html = HtmlBuilder::new("Test")
        .add_style("body {}")
        .body("<p>text</p>")
        .add_script("run();")
        .build();

    let head_end = html.find("</head>").unwrap();
    let style_pos = html.find("<style>").unwrap();
    let script_pos = html.find("<script>").unwrap();

    assert!(style_pos < head_end);
    assert!(script_pos > head_end);
}
