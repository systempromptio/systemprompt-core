use comrak::{markdown_to_html, ComrakOptions};

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
    let mut options = ComrakOptions::default();

    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.superscript = true;

    options.render.unsafe_ = false;

    let content_without_h1 = strip_first_h1(content);
    markdown_to_html(&content_without_h1, &options)
}

pub fn extract_frontmatter(content: &str) -> Option<(serde_yaml::Value, String)> {
    if !content.starts_with("---") {
        return None;
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }

    let frontmatter_str = parts[1].trim();
    let body = parts[2].to_string();

    match serde_yaml::from_str(frontmatter_str) {
        Ok(yaml) => Some((yaml, body)),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse markdown frontmatter");
            None
        },
    }
}
