//! Platform-owned `SKILL.md` frontmatter keys and the passthrough renderer.
//!
//! The platform owns seven frontmatter keys
//! ([`PLATFORM_OWNED_SKILL_KEYS`](crate::bridge::manifest::skill_frontmatter::PLATFORM_OWNED_SKILL_KEYS)):
//! `name` and `description` are re-emitted from the skill's descriptor, and
//! `title`, `tags`, `category`, `display_category` and `hosts` are catalogue
//! metadata clients do not read. Every other key is the author's and is
//! written back after `name` and `description`, keys sorted at every depth,
//! by every renderer —
//! the Claude Code bundle and the bridge's Codex, `OpenCode` and Hermes skill
//! writers alike — so they share this one implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::{Mapping, Value};

pub const PLATFORM_OWNED_SKILL_KEYS: [&str; 7] = [
    "name",
    "title",
    "description",
    "tags",
    "category",
    "display_category",
    "hosts",
];

pub fn is_platform_owned_skill_key(key: &Value) -> bool {
    key.as_str()
        .is_some_and(|key| PLATFORM_OWNED_SKILL_KEYS.contains(&key))
}

pub fn render_passthrough_frontmatter(
    frontmatter: Option<&Mapping>,
) -> Result<String, serde_yaml::Error> {
    let Some(frontmatter) = frontmatter else {
        return Ok(String::new());
    };
    let kept: Mapping = frontmatter
        .iter()
        .filter(|(key, _)| !is_platform_owned_skill_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if kept.is_empty() {
        return Ok(String::new());
    }
    serde_yaml::to_string(&sorted_mapping(&kept))
}

// Why: the manifest carries frontmatter JCS-canonical (keys sorted), so the
// gateway signing from the author's file and the bridge re-hashing from the
// manifest agree only when both render the keys in one sorted order.
fn sorted_mapping(mapping: &Mapping) -> Mapping {
    let mut entries: Vec<(&Value, &Value)> = mapping.iter().collect();
    entries.sort_by(|(a, _), (b, _)| sort_key(a).cmp(&sort_key(b)));
    entries
        .into_iter()
        .map(|(key, value)| (key.clone(), sorted_value(value)))
        .collect()
}

fn sorted_value(value: &Value) -> Value {
    match value {
        Value::Mapping(mapping) => Value::Mapping(sorted_mapping(mapping)),
        Value::Sequence(items) => Value::Sequence(items.iter().map(sorted_value).collect()),
        Value::Tagged(tagged) => Value::Tagged(Box::new(serde_yaml::value::TaggedValue {
            tag: tagged.tag.clone(),
            value: sorted_value(&tagged.value),
        })),
        other => other.clone(),
    }
}

fn sort_key(key: &Value) -> String {
    key.as_str().map_or_else(
        || serde_yaml::to_string(key).unwrap_or_default(),
        str::to_owned,
    )
}
