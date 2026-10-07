//! Platform-owned `SKILL.md` frontmatter keys and the passthrough renderer.
//!
//! The platform owns seven frontmatter keys ([`PLATFORM_OWNED_SKILL_KEYS`]):
//! `name` and `description` are re-emitted from the skill's descriptor, and
//! `title`, `tags`, `category`, `display_category` and `hosts` are catalogue
//! metadata clients do not read. Every other key is the author's and is
//! written back verbatim after `name` and `description` by every renderer —
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
    serde_yaml::to_string(&kept)
}
