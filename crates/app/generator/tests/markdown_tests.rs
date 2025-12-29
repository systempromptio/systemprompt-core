//! Unit tests for markdown processing functionality

use systemprompt_generator::{extract_frontmatter, render_markdown};

// =============================================================================
// render_markdown tests
// =============================================================================

#[test]
fn test_render_markdown() {
    let markdown = "# Hello World\n\nThis is a paragraph.";
    let html = render_markdown(markdown);

    // The first H1 should be stripped
    assert!(!html.contains("<h1>"));
    assert!(html.contains("<p>This is a paragraph.</p>"));
}

#[test]
fn test_render_markdown_preserves_subsequent_headings() {
    let markdown = "# First Heading\n\n## Second Heading\n\nContent here.";
    let html = render_markdown(markdown);

    // First H1 should be stripped, but H2 should remain
    assert!(!html.contains("<h1>"));
    assert!(html.contains("<h2>"));
    assert!(html.contains("Second Heading"));
}

#[test]
fn test_render_markdown_with_emphasis() {
    let markdown = "This is **bold** and *italic* text.";
    let html = render_markdown(markdown);

    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));
}

#[test]
fn test_markdown_with_code_blocks() {
    let markdown = r#"
Here is some code:

```rust
fn main() {
    println!("Hello, world!");
}
```

And inline `code` too.
"#;
    let html = render_markdown(markdown);

    assert!(html.contains("<pre>"));
    assert!(html.contains("<code"));
    assert!(html.contains("fn main()"));
    assert!(html.contains("<code>code</code>"));
}

#[test]
fn test_markdown_with_code_blocks_various_languages() {
    let markdown = r#"
```python
def hello():
    print("Hello")
```

```javascript
console.log("Hello");
```
"#;
    let html = render_markdown(markdown);

    assert!(html.contains("def hello()"));
    assert!(html.contains("console.log"));
}

#[test]
fn test_markdown_with_images() {
    let markdown = "![Alt text](/images/photo.jpg)";
    let html = render_markdown(markdown);

    assert!(html.contains("<img"));
    assert!(html.contains("src=\"/images/photo.jpg\""));
    assert!(html.contains("alt=\"Alt text\""));
}

#[test]
fn test_markdown_with_multiple_images() {
    let markdown = r#"
![Image 1](/img1.png)

Some text

![Image 2](/img2.jpg "Title")
"#;
    let html = render_markdown(markdown);

    assert!(html.contains("src=\"/img1.png\""));
    assert!(html.contains("src=\"/img2.jpg\""));
}

#[test]
fn test_markdown_with_links() {
    let markdown = "Check out [this link](https://example.com).";
    let html = render_markdown(markdown);

    assert!(html.contains("<a href=\"https://example.com\">this link</a>"));
}

#[test]
fn test_markdown_with_tables() {
    let markdown = r#"
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
"#;
    let html = render_markdown(markdown);

    assert!(html.contains("<table>"));
    assert!(html.contains("<th>"));
    assert!(html.contains("<td>"));
    assert!(html.contains("Header 1"));
    assert!(html.contains("Cell 1"));
}

#[test]
fn test_markdown_with_strikethrough() {
    let markdown = "This is ~~deleted~~ text.";
    let html = render_markdown(markdown);

    assert!(html.contains("<del>deleted</del>"));
}

#[test]
fn test_markdown_with_task_list() {
    let markdown = r#"
- [x] Completed task
- [ ] Pending task
"#;
    let html = render_markdown(markdown);

    assert!(html.contains("type=\"checkbox\""));
    assert!(html.contains("checked"));
}

#[test]
fn test_markdown_with_autolink() {
    let markdown = "Visit https://example.com for more info.";
    let html = render_markdown(markdown);

    assert!(html.contains("<a href=\"https://example.com\">"));
}

#[test]
fn test_markdown_empty_input() {
    let markdown = "";
    let html = render_markdown(markdown);

    assert!(html.is_empty() || html.trim().is_empty());
}

#[test]
fn test_markdown_only_heading() {
    let markdown = "# Just a heading";
    let html = render_markdown(markdown);

    // The H1 should be stripped
    assert!(!html.contains("<h1>"));
    assert!(html.trim().is_empty() || !html.contains("Just a heading"));
}

// =============================================================================
// extract_frontmatter tests
// =============================================================================

#[test]
fn test_extract_frontmatter() {
    let content = r#"---
title: Test Post
author: John Doe
---
# Content

This is the body."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("Test Post"));
    assert_eq!(yaml["author"].as_str(), Some("John Doe"));
    assert!(body.contains("# Content"));
    assert!(body.contains("This is the body."));
}

#[test]
fn test_extract_frontmatter_with_complex_yaml() {
    let content = r#"---
title: Complex Post
tags:
  - rust
  - testing
  - markdown
metadata:
  published: true
  views: 1000
---
Body content here."#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("Complex Post"));

    let tags = yaml["tags"].as_sequence().unwrap();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].as_str(), Some("rust"));

    assert_eq!(yaml["metadata"]["published"].as_bool(), Some(true));
    assert_eq!(yaml["metadata"]["views"].as_i64(), Some(1000));

    assert!(body.contains("Body content here."));
}

#[test]
fn test_extract_frontmatter_no_frontmatter() {
    let content = "# Just a heading\n\nNo frontmatter here.";

    let result = extract_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_frontmatter_incomplete() {
    let content = r#"---
title: Incomplete
# No closing delimiter
Body starts here."#;

    let result = extract_frontmatter(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_frontmatter_empty_body() {
    let content = r#"---
title: Empty Body
---
"#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, body) = result.unwrap();
    assert_eq!(yaml["title"].as_str(), Some("Empty Body"));
    assert!(body.trim().is_empty());
}

#[test]
fn test_extract_frontmatter_special_characters() {
    let content = r#"---
title: "Post with: colons & special chars"
description: "Contains \"quotes\" and 'apostrophes'"
---
Body"#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    assert!(yaml["title"].as_str().unwrap().contains("colons"));
    assert!(yaml["description"].as_str().unwrap().contains("quotes"));
}

#[test]
fn test_extract_frontmatter_date_field() {
    let content = r#"---
title: Date Test
published_at: 2024-01-15
---
Content"#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    assert!(yaml["published_at"].as_str().is_some());
}

#[test]
fn test_extract_frontmatter_multiline_string() {
    let content = r#"---
title: Multiline
description: |
  This is a
  multiline description
  that spans multiple lines.
---
Body"#;

    let result = extract_frontmatter(content);
    assert!(result.is_some());

    let (yaml, _) = result.unwrap();
    let desc = yaml["description"].as_str().unwrap();
    assert!(desc.contains("multiline description"));
}
