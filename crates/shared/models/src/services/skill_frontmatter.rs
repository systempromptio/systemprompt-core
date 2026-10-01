//! Authored `SKILL.md` frontmatter carried through to the client.
//!
//! The platform owns seven frontmatter keys ([`PLATFORM_OWNED_SKILL_KEYS`]):
//! `name` and `description` are re-emitted from the skill's descriptor, and
//! `title`, `tags`, `category`, `display_category` and `hosts` are catalogue
//! metadata Claude Code does not read. Every other key is the author's and is
//! kept as an ordered YAML mapping, then written back verbatim after `name`
//! and `description`, so a field Claude Code adds in a later release reaches
//! the client without a platform change.
//!
//! The mapping travels inside the JCS-signed manifest, so it must be
//! expressible as JSON: [`check_json_compatible`] refuses non-string mapping
//! keys, YAML tags and non-finite numbers. JCS sorts object keys, so a client
//! rendering from the manifest sees the keys alphabetically; YAML mapping order
//! carries no meaning to Claude Code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::{Mapping, Value};

use super::frontmatter::split_frontmatter;

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

#[derive(Debug, Clone, Default)]
pub struct SplitSkillFrontmatter {
    pub owned: Mapping,
    pub passthrough: Option<Mapping>,
}

pub fn split_skill_frontmatter(authored: Mapping) -> SplitSkillFrontmatter {
    let mut owned = Mapping::new();
    let mut passthrough = Mapping::new();
    for (key, value) in authored {
        if is_platform_owned_skill_key(&key) {
            owned.insert(key, value);
        } else {
            passthrough.insert(key, value);
        }
    }
    SplitSkillFrontmatter {
        owned,
        passthrough: (!passthrough.is_empty()).then_some(passthrough),
    }
}

pub fn authored_skill_frontmatter(markdown: &str) -> Result<Mapping, String> {
    let Some(front) = split_frontmatter(markdown) else {
        return Ok(Mapping::new());
    };
    match serde_yaml::from_str::<Value>(front.yaml)
        .map_err(|e| format!("SKILL.md frontmatter is not valid YAML: {e}"))?
    {
        Value::Null => Ok(Mapping::new()),
        Value::Mapping(mapping) => Ok(mapping),
        _ => Err("SKILL.md frontmatter must be a YAML mapping".to_owned()),
    }
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

pub fn check_json_compatible(frontmatter: &Mapping) -> Result<(), String> {
    for (key, value) in frontmatter {
        let Some(key) = key.as_str() else {
            return Err(format!("frontmatter key {key:?} is not a string"));
        };
        check_value(key, value)?;
    }
    Ok(())
}

fn check_value(path: &str, value: &Value) -> Result<(), String> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => {
            if number.as_f64().is_some_and(|n| !n.is_finite()) {
                return Err(format!("frontmatter '{path}' is not a finite number"));
            }
            Ok(())
        },
        Value::Sequence(items) => items.iter().try_for_each(|item| check_value(path, item)),
        Value::Mapping(map) => map.iter().try_for_each(|(key, item)| {
            key.as_str().map_or_else(
                || {
                    Err(format!(
                        "frontmatter '{path}' has a key that is not a string"
                    ))
                },
                |key| check_value(&format!("{path}.{key}"), item),
            )
        }),
        Value::Tagged(_) => Err(format!("frontmatter '{path}' carries a YAML tag")),
    }
}
