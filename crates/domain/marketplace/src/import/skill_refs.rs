//! What a skill body cites but the imported tree will not carry.
//!
//! A `references/…` path the body names but the copied skill tree lacks (never
//! authored, or dropped as a dev file) reaches no client, and a `../` path
//! cannot resolve once composition renames skill directories to `snake_case`.
//! Both are reported per skill so a strict import refuses the tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::SkillId;

use super::skill::SKILL_FILE;
use super::warning::ImportWarning;
use super::writer::Sink;

const REFERENCES_PREFIX: &str = "references/";
const PARENT_PREFIX: &str = "../";

pub(super) fn skill_body_warnings(
    id: &SkillId,
    skill_dir: &Path,
    sink: &Sink,
) -> Vec<ImportWarning> {
    let Ok(body) = std::fs::read_to_string(skill_dir.join(SKILL_FILE)) else {
        return Vec::new();
    };
    let copied = Path::new("skills").join(id.as_str());
    let mut missing = BTreeSet::new();
    let mut parent = BTreeSet::new();
    for path in cited_paths(&body) {
        if path.starts_with(PARENT_PREFIX) {
            parent.insert(path.to_owned());
        } else if path.starts_with(REFERENCES_PREFIX) && !sink.exists(&copied.join(path)) {
            missing.insert(path.to_owned());
        }
    }
    let skill = id.as_str().to_owned();
    missing
        .into_iter()
        .map(|path| ImportWarning::MissingReference {
            skill: skill.clone(),
            path,
        })
        .chain(
            parent
                .into_iter()
                .map(|path| ImportWarning::SiblingSkillPath {
                    skill: skill.clone(),
                    path,
                }),
        )
        .collect()
}

pub(super) fn skill_dirs_without_manifest(plugin_dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(plugin_dir.join("skills")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.join(SKILL_FILE).is_file())
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_owned))
        .collect();
    out.sort();
    out
}

fn cited_paths(body: &str) -> impl Iterator<Item = &str> {
    body.split(|c: char| c.is_whitespace() || "()[]<>`'\"".contains(c))
        .map(|token| token.trim_end_matches(['.', ',', ';', ':']))
        .map(|token| token.split(['#', '?']).next().unwrap_or(token))
        .filter(|token| {
            token.len() > REFERENCES_PREFIX.len() && token.starts_with(REFERENCES_PREFIX)
                || token.starts_with(PARENT_PREFIX)
        })
}
