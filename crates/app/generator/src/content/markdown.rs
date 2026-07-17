//! Markdown rendering and YAML frontmatter extraction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use comrak::{Options, markdown_to_html};
use systemprompt_models::split_frontmatter;

fn strip_first_h1(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut found_h1 = false;

    for line in lines {
        let trimmed = line.trim();
        if !found_h1 && trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
            found_h1 = true;
            continue;
        }
        result.push(line);
    }

    result.join("\n")
}

pub fn render_markdown(content: &str) -> String {
    let mut options = Options::default();

    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.superscript = true;

    options.render.r#unsafe = false;

    let content_without_h1 = strip_first_h1(content);
    markdown_to_html(&content_without_h1, &options)
}

pub fn extract_frontmatter(content: &str) -> Option<(serde_yaml::Value, String)> {
    let frontmatter = split_frontmatter(content)?;
    let body = frontmatter.body.to_owned();

    match serde_yaml::from_str(frontmatter.yaml.trim()) {
        Ok(yaml) => Some((yaml, body)),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse markdown frontmatter");
            None
        },
    }
}
