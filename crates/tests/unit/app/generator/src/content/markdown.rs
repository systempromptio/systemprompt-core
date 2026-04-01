//! Unit tests for markdown processing

use systemprompt_generator::{extract_frontmatter, render_markdown};

// ============================================================================
// render_markdown Tests
// ============================================================================

#[test]
fn test_render_markdown_basic_paragraph() {
    let input = "This is a paragraph.";
    let result = render_markdown(input);
    assert!(result.contains("<p>"));
    assert!(result.contains("This is a paragraph."));
    assert!(result.contains("</p>"));
}

#[test]
fn test_render_markdown_strips_first_h1() {
    let input = "# Title\n\nSome content here.";
    let result = render_markdown(input);
    // The first h1 should be stripped
    assert!(!result.contains("<h1>"));
    assert!(!result.contains("Title"));
    assert!(result.contains("Some content here."));
}

#[test]
fn test_render_markdown_preserves_h2() {
    let input = "## Subtitle\n\nContent after h2.";
    let result = render_markdown(input);
    assert!(result.contains("<h2>"));
    assert!(result.contains("Subtitle"));
}

#[test]
fn test_render_markdown_preserves_multiple_h1_after_first() {
    let input = "# First Title\n\n# Second Title\n\nContent.";
    let result = render_markdown(input);
    // First h1 stripped, second preserved (but as h1 since it starts with "# ")
    assert!(!result.contains("First Title"));
    assert!(result.contains("Second Title"));
}

#[test]
fn test_render_markdown_bold() {
    let input = "This is **bold** text.";
    let result = render_markdown(input);
    assert!(result.contains("<strong>bold</strong>"));
}

#[test]
fn test_render_markdown_italic() {
    let input = "This is *italic* text.";
    let result = render_markdown(input);
    assert!(result.contains("<em>italic</em>"));
}

#[test]
fn test_render_markdown_strikethrough() {
    let input = "This is ~~strikethrough~~ text.";
    let result = render_markdown(input);
    assert!(result.contains("<del>strikethrough</del>"));
}

#[test]
fn test_render_markdown_code_inline() {
    let input = "Use `code` here.";
    let result = render_markdown(input);
    assert!(result.contains("<code>code</code>"));
}

#[test]
fn test_render_markdown_code_block() {
    let input = "```rust\nfn main() {}\n```";
    let result = render_markdown(input);
    assert!(result.contains("<pre>"));
    assert!(result.contains("<code"));
    assert!(result.contains("fn main()"));
}

#[test]
fn test_render_markdown_unordered_list() {
    let input = "- Item 1\n- Item 2\n- Item 3";
    let result = render_markdown(input);
    assert!(result.contains("<ul>"));
    assert!(result.contains("<li>"));
    assert!(result.contains("Item 1"));
    assert!(result.contains("Item 2"));
    assert!(result.contains("Item 3"));
}

#[test]
fn test_render_markdown_ordered_list() {
    let input = "1. First\n2. Second\n3. Third";
    let result = render_markdown(input);
    assert!(result.contains("<ol>"));
    assert!(result.contains("<li>"));
}

#[test]
fn test_render_markdown_table() {
    let input = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
    let result = render_markdown(input);
    assert!(result.contains("<table>"));
    assert!(result.contains("<th>"));
    assert!(result.contains("<td>"));
    assert!(result.contains("Header 1"));
    assert!(result.contains("Cell 1"));
}

#[test]
fn test_render_markdown_link() {
    let input = "Click [here](https://example.com) for more.";
    let result = render_markdown(input);
    assert!(result.contains("<a href=\"https://example.com\">here</a>"));
}

#[test]
fn test_render_markdown_autolink() {
    let input = "Visit https://example.com for info.";
    let result = render_markdown(input);
    assert!(result.contains("<a href=\"https://example.com\">"));
}

#[test]
fn test_render_markdown_image() {
    let input = "![Alt text](/images/photo.jpg)";
    let result = render_markdown(input);
    assert!(result.contains("<img"));
    assert!(result.contains("src=\"/images/photo.jpg\""));
    assert!(result.contains("alt=\"Alt text\""));
}

#[test]
fn test_render_markdown_tasklist() {
    let input = "- [x] Completed\n- [ ] Not done";
    let result = render_markdown(input);
    assert!(result.contains("type=\"checkbox\""));
    assert!(result.contains("checked"));
}

#[test]
fn test_render_markdown_superscript() {
    let input = "E = mc^2^";
    let result = render_markdown(input);
    assert!(result.contains("<sup>2</sup>"));
}

#[test]
fn test_render_markdown_blockquote() {
    let input = "> This is a quote.";
    let result = render_markdown(input);
    assert!(result.contains("<blockquote>"));
    assert!(result.contains("This is a quote."));
}

#[test]
fn test_render_markdown_horizontal_rule() {
    let input = "Before\n\n---\n\nAfter";
    let result = render_markdown(input);
    assert!(result.contains("<hr"));
}

#[test]
fn test_render_markdown_empty_input() {
    let input = "";
    let result = render_markdown(input);
    assert!(result.is_empty() || result.trim().is_empty());
}

#[test]
fn test_render_markdown_only_whitespace() {
    let input = "   \n\n   ";
    let result = render_markdown(input);
    assert!(result.trim().is_empty());
}

#[test]
fn test_render_markdown_nested_formatting() {
    let input = "This is ***bold and italic*** text.";
    let result = render_markdown(input);
    assert!(result.contains("<strong>") || result.contains("<em>"));
}

// ============================================================================
// extract_frontmatter Tests
// ============================================================================

#[test]
fn test_extract_frontmatter_valid() {
    let content = r#"---
title: My Title
date: 2024-01-15
---
Body content here."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("My Title"));
    assert_eq!(yaml["date"].as_str(), Some("2024-01-15"));
    assert!(body.contains("Body content here."));
}

#[test]
fn test_extract_frontmatter_no_frontmatter() {
    let content = "Just some content without frontmatter.";
    let result = extract_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_frontmatter_incomplete_delimiters() {
    let content = r#"---
title: Test
No closing delimiter"#;

    let result = extract_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_frontmatter_empty_frontmatter() {
    let content = r#"---
---
Body content."#;

    let result = extract_frontmatter(content);
    // Empty frontmatter should still parse (as empty YAML)
    assert!(result.is_some());
}

#[test]
fn test_extract_frontmatter_complex_yaml() {
    let content = r#"---
title: Complex Title
tags:
  - rust
  - testing
  - generator
metadata:
  author: Test Author
  version: 1.0
---
Content body."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("Complex Title"));
    assert!(yaml["tags"].as_sequence().is_some());
    assert!(yaml["metadata"]["author"].as_str().is_some());
    assert!(body.contains("Content body."));
}

#[test]
fn test_extract_frontmatter_with_special_characters() {
    let content = r#"---
title: "Title with: colons & special chars"
description: |
  Multi-line
  description here
---
Body."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    assert!(yaml["title"].as_str().unwrap().contains("colons"));
}

#[test]
fn test_extract_frontmatter_invalid_yaml() {
    let content = r#"---
title: Valid
invalid yaml: [not closed
---
Body."#;

    let result = extract_frontmatter(content);
    // Invalid YAML should return None
    assert!(result.is_none());
}

#[test]
fn test_extract_frontmatter_with_markdown_in_body() {
    let content = r#"---
title: Test
---
# Heading

Some **bold** text and a [link](http://example.com)."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (_, body) = result.unwrap();
    assert!(body.contains("# Heading"));
    assert!(body.contains("**bold**"));
}

#[test]
fn test_extract_frontmatter_numbers_and_booleans() {
    let content = r#"---
count: 42
rating: 4.5
published: true
draft: false
---
Content."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    assert_eq!(yaml["count"].as_i64(), Some(42));
    assert_eq!(yaml["rating"].as_f64(), Some(4.5));
    assert_eq!(yaml["published"].as_bool(), Some(true));
    assert_eq!(yaml["draft"].as_bool(), Some(false));
}

#[test]
fn test_extract_frontmatter_dates() {
    let content = r#"---
created: 2024-01-15
updated: 2024-01-20T10:30:00Z
---
Body."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    // Dates are typically parsed as strings in serde_yaml
    assert!(yaml["created"].as_str().is_some() || yaml["created"].is_string());
}

#[test]
fn test_extract_frontmatter_whitespace_around_delimiters() {
    let content = r#"---
title: Test
---


Body with leading whitespace."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (_, body) = result.unwrap();
    // Body should preserve whitespace but may be trimmed depending on implementation
    assert!(body.contains("Body with leading whitespace."));
}

#[test]
fn test_extract_frontmatter_triple_dash_in_body() {
    let content = r#"---
title: Test
---
Some content with --- dashes in the middle."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (_, body) = result.unwrap();
    assert!(body.contains("---"));
}
