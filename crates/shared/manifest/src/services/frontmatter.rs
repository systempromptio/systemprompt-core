//! Line-anchored YAML frontmatter splitting.
//!
//! The canonical frontmatter parser for every consumer in the workspace
//! (skills, content ingestion, sync diffing, static generation). A
//! frontmatter block opens with a `---` line at the very start of the
//! document (after an optional UTF-8 BOM) and closes at the next line that
//! is exactly `---`. A `---` anywhere else — mid-line, in a markdown table
//! separator row, or as a horizontal rule — is body text, never a delimiter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy)]
pub struct Frontmatter<'a> {
    pub yaml: &'a str,
    pub body: &'a str,
}

pub fn split_frontmatter(content: &str) -> Option<Frontmatter<'_>> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.split_inclusive('\n');

    let opening = lines.next()?;
    if opening.trim_end() != "---" {
        return None;
    }

    let yaml_start = opening.len();
    let mut offset = yaml_start;
    for line in lines {
        if line.trim_end() == "---" {
            return Some(Frontmatter {
                yaml: &content[yaml_start..offset],
                body: &content[offset + line.len()..],
            });
        }
        offset += line.len();
    }
    None
}

pub fn strip_frontmatter(content: &str) -> String {
    split_frontmatter(content).map_or_else(|| content.to_owned(), |f| f.body.trim().to_owned())
}
