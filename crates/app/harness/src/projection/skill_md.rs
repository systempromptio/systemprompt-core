//! `SKILL.md` bytes as the bridge renders them for a host whose skill loader
//! requires the front matter's `name` to match the skill's directory, and the
//! version recorded in the skills ledger.
//!
//! A body without front matter gets `name` (the directory), `description`
//! (double-quoted when YAML would read it as something else, escaping only `"`;
//! an empty one is `""`, which a loader reads as a string rather than null),
//! the manifest's own front matter keys sorted at every depth minus the ones
//! the platform owns, a blank line and the body. A body with front matter of
//! its own keeps its lines, minus blank ones and any `name:`, after a `name:`
//! naming the directory. The file always ends in a newline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};
use sp_protocol::names::sha256_hex;
use sp_protocol::wire::manifest::SkillEntry;

use crate::projection::model::SkillFile;

const PLATFORM_KEYS: [&str; 7] = [
    "name",
    "title",
    "description",
    "tags",
    "category",
    "display_category",
    "hosts",
];

const VERSION_LEN: usize = 16;

const FRONT_MATTER_END: &str = "\n---";

pub(crate) fn render(skill: &SkillEntry, dir: &str) -> Result<String, serde_yaml::Error> {
    let body = skill.instructions.as_deref().unwrap_or_default();
    if let Some(markdown) = with_own_front_matter(body, dir) {
        return Ok(markdown);
    }
    let passthrough = passthrough(skill.frontmatter.as_ref())?;
    let description = scalar(skill.description.as_deref().unwrap_or_default());
    Ok(ending_newline(format!(
        "---\nname: {dir}\ndescription: {description}\n{passthrough}---\n\n{body}"
    )))
}

pub(crate) fn version(files: &[SkillFile]) -> String {
    let mut buffer = String::new();
    for file in files {
        buffer.push_str(&file.dir);
        buffer.push('\0');
        buffer.push_str(&file.markdown);
        buffer.push('\0');
    }
    let digest = sha256_hex(buffer.as_bytes());
    digest
        .get(..VERSION_LEN)
        .unwrap_or(digest.as_str())
        .to_owned()
}

fn with_own_front_matter(body: &str, dir: &str) -> Option<String> {
    let rest = body.trim_start().strip_prefix("---")?;
    let end = rest.find(FRONT_MATTER_END)?;
    let block = rest.get(..end)?;
    let tail = rest.get(end + FRONT_MATTER_END.len()..)?;
    let mut lines = vec![format!("name: {dir}")];
    lines.extend(
        block
            .lines()
            .filter(|line| !line.trim_start().starts_with("name:") && !line.trim().is_empty())
            .map(str::to_owned),
    );
    Some(ending_newline(format!(
        "---\n{}\n---{tail}",
        lines.join("\n")
    )))
}

fn scalar(text: &str) -> String {
    if text.is_empty() {
        return "\"\"".to_owned();
    }
    let quoted = text.contains(':')
        || text.contains('#')
        || text.starts_with(['-', '?', '!', '&', '*', '|', '>', '\'', '"', '%', '@', '`']);
    if quoted {
        format!("\"{}\"", text.replace('"', "\\\""))
    } else {
        text.to_owned()
    }
}

// JSON: a skill's front matter as the manifest carries it, rendered to YAML key
// by key.
fn passthrough(frontmatter: Option<&Map<String, Value>>) -> Result<String, serde_yaml::Error> {
    let Some(frontmatter) = frontmatter else {
        return Ok(String::new());
    };
    let mut kept: Vec<(&String, &Value)> = frontmatter
        .iter()
        .filter(|(key, _)| !PLATFORM_KEYS.contains(&key.as_str()))
        .collect();
    if kept.is_empty() {
        return Ok(String::new());
    }
    kept.sort_by(|a, b| a.0.cmp(b.0));
    let sorted: Map<String, Value> = kept
        .into_iter()
        .map(|(key, value)| (key.clone(), sorted_value(value)))
        .collect();
    let text = serde_yaml::to_string(&Value::Object(sorted))?;
    if let Some(rest) = text.strip_prefix("---\n") {
        return Ok(rest.to_owned());
    }
    Ok(text)
}

// JSON: a front matter value of any shape, copied with its object keys sorted
// at every depth.
fn sorted_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), sorted_value(value)))
                    .collect(),
            )
        },
        Value::Array(items) => Value::Array(items.iter().map(sorted_value).collect()),
        other => other.clone(),
    }
}

fn ending_newline(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}
