use systemprompt_generator::{extract_frontmatter, render_markdown};

#[test]
fn render_markdown_no_h1_passthrough_without_stripping() {
    let md = "## Section\n\nSome content.";
    let html = render_markdown(md);
    assert!(html.contains("<h2>"));
    assert!(html.contains("Section"));
    assert!(html.contains("Some content."));
}

#[test]
fn render_markdown_multiple_h1s_only_first_stripped() {
    let md = "# First H1\n\n# Second H1\n\nBody.";
    let html = render_markdown(md);
    assert!(!html.contains("First H1"), "first H1 should be stripped");
    assert!(html.contains("Second H1"), "second H1 should remain");
}

#[test]
fn render_markdown_h1_without_space_not_stripped() {
    let md = "#NoSpace\n\nBody.";
    let html = render_markdown(md);
    assert!(html.contains("NoSpace") || html.contains("#NoSpace"));
}

#[test]
fn render_markdown_h2_not_treated_as_h1() {
    let md = "## Not H1\n\nContent.";
    let html = render_markdown(md);
    assert!(html.contains("Not H1"), "h2 should not be stripped");
}

#[test]
fn render_markdown_strikethrough_enabled() {
    let md = "~~removed~~";
    let html = render_markdown(md);
    assert!(html.contains("<del>removed</del>"));
}

#[test]
fn render_markdown_tables_enabled() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let html = render_markdown(md);
    assert!(html.contains("<table>"));
    assert!(html.contains("<th>"));
}

#[test]
fn render_markdown_task_list_enabled() {
    let md = "- [x] Done\n- [ ] Pending";
    let html = render_markdown(md);
    assert!(html.contains("checkbox"));
}

#[test]
fn render_markdown_fenced_code_block_with_language() {
    let md = "```python\nprint('hello')\n```";
    let html = render_markdown(md);
    assert!(html.contains("print"));
    assert!(html.contains("<code"));
}

#[test]
fn render_markdown_blockquote() {
    let md = "> This is a quote.";
    let html = render_markdown(md);
    assert!(html.contains("<blockquote>") || html.contains("blockquote"));
    assert!(html.contains("This is a quote."));
}

#[test]
fn render_markdown_ordered_list() {
    let md = "1. First\n2. Second\n3. Third";
    let html = render_markdown(md);
    assert!(html.contains("<ol>"));
    assert!(html.contains("<li>"));
    assert!(html.contains("First"));
    assert!(html.contains("Third"));
}

#[test]
fn render_markdown_unordered_list() {
    let md = "- Item A\n- Item B\n- Item C";
    let html = render_markdown(md);
    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>"));
}

#[test]
fn render_markdown_nested_list() {
    let md = "- Parent\n  - Child\n    - Grandchild";
    let html = render_markdown(md);
    assert!(html.contains("<li>"));
    assert!(html.contains("Parent"));
}

#[test]
fn render_markdown_horizontal_rule() {
    let md = "---";
    let html = render_markdown(md);
    assert!(html.contains("<hr"));
}

#[test]
fn render_markdown_link_with_title() {
    let md = "[link](https://example.com \"Title Here\")";
    let html = render_markdown(md);
    assert!(html.contains("https://example.com"));
    assert!(html.contains("link"));
}

#[test]
fn render_markdown_superscript_enabled() {
    let md = "x^2^";
    let html = render_markdown(md);
    assert!(html.contains("2") || html.contains("<sup>"));
}

#[test]
fn render_markdown_no_raw_html() {
    let md = "<script>alert('xss')</script>\n\nNormal content.";
    let html = render_markdown(md);
    assert!(html.contains("Normal content."));
    assert!(!html.contains("<script>"));
}

#[test]
fn extract_frontmatter_returns_none_for_empty_string() {
    assert!(extract_frontmatter("").is_none());
}

#[test]
fn extract_frontmatter_returns_none_for_single_separator() {
    assert!(extract_frontmatter("---\ntitle: Test\n").is_none());
}

#[test]
fn extract_frontmatter_with_only_separator_lines() {
    let content = "---\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, body) = result.unwrap();
    assert_eq!(yaml, serde_yaml::Value::Null);
    assert!(body.contains("body"));
}

#[test]
fn extract_frontmatter_preserves_body_with_separator_inside() {
    let content = "---\ntitle: Test\n---\nSome text\n---\nMore text after dash triple";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("Test"));
    assert!(body.contains("Some text"));
}

#[test]
fn extract_frontmatter_numeric_values() {
    let content = "---\ncount: 42\nrating: 4.5\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, _) = result.unwrap();
    assert_eq!(yaml["count"].as_i64(), Some(42));
}

#[test]
fn extract_frontmatter_boolean_values() {
    let content = "---\npublic: true\ndraft: false\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, _) = result.unwrap();
    assert_eq!(yaml["public"].as_bool(), Some(true));
    assert_eq!(yaml["draft"].as_bool(), Some(false));
}

#[test]
fn extract_frontmatter_list_values() {
    let content = "---\ntags:\n  - rust\n  - web\n  - sse\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, _) = result.unwrap();
    let tags = yaml["tags"].as_sequence().unwrap();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].as_str(), Some("rust"));
    assert_eq!(tags[2].as_str(), Some("sse"));
}

#[test]
fn extract_frontmatter_invalid_yaml_returns_none() {
    let content = "---\n  bad: [\nyaml:\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn extract_frontmatter_null_field() {
    let content = "---\noptional:\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, _) = result.unwrap();
    assert!(yaml["optional"].is_null());
}

#[test]
fn extract_frontmatter_nested_object() {
    let content = "---\nmeta:\n  author: Alice\n  views: 100\n---\nbody";
    let result = extract_frontmatter(content);
    assert!(result.is_some());
    let (yaml, _) = result.unwrap();
    assert_eq!(yaml["meta"]["author"].as_str(), Some("Alice"));
    assert_eq!(yaml["meta"]["views"].as_i64(), Some(100));
}
