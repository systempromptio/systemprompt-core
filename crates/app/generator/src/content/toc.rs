use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

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
        let open_tag = format!("<h{}", entry.level);
        let close_tag = format!("</h{}>", entry.level);

        if let Some(tag_start) =
            find_heading_position(&result, &open_tag, &close_tag, &entry.text)
        {
            let close_bracket = result[tag_start..].find('>').map(|i| tag_start + i);
            if let Some(cb) = close_bracket {
                let attrs = &result[tag_start + open_tag.len()..cb];
                if !attrs.contains("id=") {
                    let id_attr = format!(" id=\"{}\"", entry.slug);
                    result.insert_str(tag_start + open_tag.len(), &id_attr);
                }
            }
        }
    }

    result
}

fn find_heading_position(
    html: &str,
    open_tag: &str,
    close_tag: &str,
    heading_text: &str,
) -> Option<usize> {
    let mut search_start = 0;
    while let Some(pos) = html[search_start..].find(open_tag) {
        let abs_pos = search_start + pos;
        if let Some(close_pos) = html[abs_pos..].find(close_tag) {
            let segment = &html[abs_pos..abs_pos + close_pos + close_tag.len()];
            if segment.contains(heading_text) {
                return Some(abs_pos);
            }
        }
        search_start = abs_pos + open_tag.len();
    }
    None
}
