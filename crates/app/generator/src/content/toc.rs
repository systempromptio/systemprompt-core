use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, Options};

#[derive(Debug)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

#[derive(Debug)]
pub struct TocResult {
    pub toc_html: String,
    pub content_html: String,
}

pub fn generate_toc(markdown: &str, rendered_html: &str) -> TocResult {
    let entries = extract_headings(markdown);

    if entries.is_empty() {
        return TocResult {
            toc_html: String::new(),
            content_html: rendered_html.to_string(),
        };
    }

    let toc_html = build_toc_html(&entries);
    let content_html = inject_heading_ids(rendered_html, &entries);

    TocResult {
        toc_html,
        content_html,
    }
}

fn extract_headings(markdown: &str) -> Vec<TocEntry> {
    let arena = Arena::new();
    let options = Options::default();
    let root = parse_document(&arena, markdown, &options);

    let mut entries = Vec::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut skip_first_h1 = true;

    for node in root.descendants() {
        if let NodeValue::Heading(heading) = &node.data.borrow().value {
            let level = heading.level;

            if level == 1 && skip_first_h1 {
                skip_first_h1 = false;
                continue;
            }

            if !(2..=6).contains(&level) {
                continue;
            }

            let text = extract_text_from_node(node);
            if text.is_empty() {
                continue;
            }

            let base_slug = slugify(&text);
            let slug = deduplicate_slug(&base_slug, &mut slug_counts);

            entries.push(TocEntry { level, text, slug });
        }
    }

    entries
}

fn extract_text_from_node<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();

    for child in node.descendants() {
        if let NodeValue::Text(ref content) = child.data.borrow().value {
            text.push_str(content);
        } else if let NodeValue::Code(ref code) = child.data.borrow().value {
            text.push_str(&code.literal);
        }
    }

    text.trim().to_string()
}

fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn deduplicate_slug(base_slug: &str, counts: &mut HashMap<String, usize>) -> String {
    let count = counts.entry(base_slug.to_string()).or_insert(0);
    let slug = if *count == 0 {
        base_slug.to_string()
    } else {
        format!("{}-{}", base_slug, count)
    };
    *count += 1;
    slug
}

fn build_toc_html(entries: &[TocEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut html = String::new();
    let mut stack: Vec<u8> = Vec::new();

    html.push_str("<ul class=\"toc-list\">\n");
    stack.push(entries[0].level);

    for entry in entries {
        while let Some(&current_level) = stack.last() {
            match entry.level.cmp(&current_level) {
                std::cmp::Ordering::Greater => {
                    html.push_str("<ul class=\"toc-list toc-nested\">\n");
                    stack.push(entry.level);
                    break;
                },
                std::cmp::Ordering::Less => {
                    html.push_str("</li>\n</ul>\n");
                    stack.pop();
                },
                std::cmp::Ordering::Equal => {
                    if html.ends_with("</a>\n") {
                        html.push_str("</li>\n");
                    }
                    break;
                },
            }
        }

        html.push_str(&format!(
            "<li class=\"toc-item toc-level-{}\">\n<a class=\"toc-link\" href=\"#{}\">{}</a>\n",
            entry.level,
            entry.slug,
            escape_html(&entry.text)
        ));
    }

    while !stack.is_empty() {
        html.push_str("</li>\n</ul>\n");
        stack.pop();
    }

    html
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn inject_heading_ids(html: &str, entries: &[TocEntry]) -> String {
    let mut result = html.to_string();

    for entry in entries {
        let escaped_text = regex_escape(&entry.text);

        for tag in ["h2", "h3", "h4", "h5", "h6"] {
            let pattern = format!(r"<{}([^>]*)>([^<]*{}[^<]*)</{}>", tag, escaped_text, tag);

            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re
                    .replace(&result, |caps: &regex::Captures| {
                        let attrs = caps.get(1).map_or("", |m| m.as_str());
                        let content = caps.get(2).map_or("", |m| m.as_str());

                        if attrs.contains("id=") {
                            format!("<{}{}>{}</{}>", tag, attrs, content, tag)
                        } else {
                            format!(
                                "<{} id=\"{}\"{}>{}</{}>",
                                tag, entry.slug, attrs, content, tag
                            )
                        }
                    })
                    .to_string();
            }
        }
    }

    result
}

fn regex_escape(text: &str) -> String {
    let special_chars = [
        '\\', '.', '+', '*', '?', '(', ')', '[', ']', '{', '}', '^', '$', '|',
    ];
    let mut result = String::new();
    for c in text.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("What's New?"), "what-s-new");
        assert_eq!(slugify("OAuth2/OIDC"), "oauth2-oidc");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
    }

    #[test]
    fn test_deduplicate_slug() {
        let mut counts = HashMap::new();
        assert_eq!(deduplicate_slug("section", &mut counts), "section");
        assert_eq!(deduplicate_slug("section", &mut counts), "section-1");
        assert_eq!(deduplicate_slug("section", &mut counts), "section-2");
        assert_eq!(deduplicate_slug("other", &mut counts), "other");
    }

    #[test]
    fn test_extract_headings_skips_h1() {
        let markdown = "# Title\n\n## Section One\n\n### Subsection\n\n## Section Two";
        let entries = extract_headings(markdown);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text, "Section One");
        assert_eq!(entries[0].level, 2);
        assert_eq!(entries[1].text, "Subsection");
        assert_eq!(entries[1].level, 3);
        assert_eq!(entries[2].text, "Section Two");
        assert_eq!(entries[2].level, 2);
    }

    #[test]
    fn test_empty_markdown() {
        let entries = extract_headings("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_no_headings() {
        let markdown = "Just some text without any headings.";
        let entries = extract_headings(markdown);
        assert!(entries.is_empty());
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_real_content_with_code_block() {
        let markdown = r#"# Layer System

SystemPrompt organizes its 33 crates into five distinct layers.

## Layer Diagram

```
┌────────────────────────────────────────────┐
│                 ENTRY LAYER                 │
└────────────────────────────────────────────┘
```

## Shared Layer

The foundation layer.

### Models

Core data structures.
"#;

        let entries = extract_headings(markdown);
        println!("Entries found: {:?}", entries);

        assert!(!entries.is_empty(), "Should find headings");
        assert_eq!(
            entries.len(),
            3,
            "Should find 3 headings (Layer Diagram, Shared Layer, Models)"
        );
        assert_eq!(entries[0].text, "Layer Diagram");
        assert_eq!(entries[0].level, 2);
    }
}
