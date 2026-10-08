//! Authored `SKILL.md` frontmatter carried through to the client.
//!
//! The platform owns seven frontmatter keys
//! (`systemprompt_models::bridge::manifest::skill_frontmatter::PLATFORM_OWNED_SKILL_KEYS`):
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
use thiserror::Error;

use systemprompt_models::bridge::manifest::skill_frontmatter::is_platform_owned_skill_key;

use super::frontmatter::split_frontmatter;

#[derive(Debug, Error)]
pub enum SkillFrontmatterError {
    #[error("SKILL.md frontmatter is not valid YAML: {0}")]
    Yaml(#[source] serde_yaml::Error),

    #[error("SKILL.md frontmatter must be a YAML mapping")]
    NotMapping,

    #[error("frontmatter key {0:?} is not a string")]
    NonStringKey(Box<Value>),

    #[error("frontmatter '{0}' is not a finite number")]
    NonFiniteNumber(String),

    #[error("frontmatter '{0}' has a key that is not a string")]
    NonStringNestedKey(String),

    #[error("frontmatter '{0}' carries a YAML tag")]
    Tagged(String),
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

pub fn authored_skill_frontmatter(markdown: &str) -> Result<Mapping, SkillFrontmatterError> {
    let Some(front) = split_frontmatter(markdown) else {
        return Ok(Mapping::new());
    };
    match serde_yaml::from_str::<Value>(front.yaml).map_err(SkillFrontmatterError::Yaml)? {
        Value::Null => Ok(Mapping::new()),
        Value::Mapping(mapping) => Ok(mapping),
        _ => Err(SkillFrontmatterError::NotMapping),
    }
}

pub fn check_json_compatible(frontmatter: &Mapping) -> Result<(), SkillFrontmatterError> {
    for (key, value) in frontmatter {
        let Some(key_str) = key.as_str() else {
            return Err(SkillFrontmatterError::NonStringKey(Box::new(key.clone())));
        };
        check_value(key_str, value)?;
    }
    Ok(())
}

fn check_value(path: &str, value: &Value) -> Result<(), SkillFrontmatterError> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => {
            if number.as_f64().is_some_and(|n| !n.is_finite()) {
                return Err(SkillFrontmatterError::NonFiniteNumber(path.to_owned()));
            }
            Ok(())
        },
        Value::Sequence(items) => items.iter().try_for_each(|item| check_value(path, item)),
        Value::Mapping(map) => map.iter().try_for_each(|(key, item)| {
            key.as_str().map_or_else(
                || Err(SkillFrontmatterError::NonStringNestedKey(path.to_owned())),
                |key| check_value(&format!("{path}.{key}"), item),
            )
        }),
        Value::Tagged(_) => Err(SkillFrontmatterError::Tagged(path.to_owned())),
    }
}
