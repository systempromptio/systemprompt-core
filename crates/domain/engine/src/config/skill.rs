//! Skills: `<root>/<name>/SKILL.md` with a name and description in front matter
//! and instructions in the body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::error::{ConfigError, FieldError};
use crate::frontmatter;
use crate::fsio::Probe;

pub(crate) const SKILL_FILE: &str = "SKILL.md";

/// A skill found on disk: `location` is its `SKILL.md`, `dir` the directory
/// holding the files its body refers to.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub location: PathBuf,
    pub dir: PathBuf,
    pub body: String,
    // JSON: front matter keys beyond name and description, kept as written.
    pub extra: Map<String, Value>,
}

pub(crate) fn discover(
    probe: &mut Probe,
    roots: &[PathBuf],
    problems: &mut Vec<ConfigError>,
) -> IndexMap<String, Skill> {
    let mut skills = IndexMap::new();
    for root in roots {
        for entry in probe.list(root) {
            if entry.name.starts_with('.') || !entry.is_dir {
                continue;
            }
            match read_skill(probe, &entry.path, &entry.name) {
                Ok(Some(skill)) => {
                    if let Some(previous) = skills.insert(skill.name.clone(), skill) {
                        tracing::debug!(
                            skill = %previous.name,
                            shadowed = %previous.location.display(),
                            "skill replaced by a nearer one"
                        );
                    }
                },
                Ok(None) => {},
                Err(problem) => problems.push(problem),
            }
        }
    }
    skills.sort_keys();
    skills
}

fn read_skill(probe: &mut Probe, dir: &Path, dir_name: &str) -> Result<Option<Skill>, ConfigError> {
    let location = dir.join(SKILL_FILE);
    let Some(text) = probe.read(&location)? else {
        return Ok(None);
    };
    let document = frontmatter::parse(&location, &text)?;
    let mut data = document.data;
    let name = required(&location, &mut data, "name")?;
    if name != dir_name {
        return Err(ConfigError::SkillName {
            path: location,
            name,
            dir: dir_name.to_owned(),
        });
    }
    let description = required(&location, &mut data, "description")?;
    Ok(Some(Skill {
        name,
        description,
        dir: dir.to_path_buf(),
        location,
        body: document.body,
        extra: data,
    }))
}

// JSON: the front matter block, from which the field is taken.
fn required(
    path: &Path,
    data: &mut Map<String, Value>,
    field: &'static str,
) -> Result<String, ConfigError> {
    match data.shift_remove(field) {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.trim().to_owned()),
        Some(Value::String(_)) | None => Err(ConfigError::missing(path, field)),
        Some(_) => Err(ConfigError::field(
            path,
            field,
            FieldError::Type {
                expected: "a string",
            },
        )),
    }
}
