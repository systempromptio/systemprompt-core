//! Skill component projection: selecting a plugin's skills from the resolved
//! catalogue and laying them out as `skills/<kebab>/SKILL.md` plus auxiliary
//! files (`scripts/`, `references/`, …).

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::AgentId;
use systemprompt_models::bridge::ids::SkillId;
use systemprompt_models::bridge::manifest::SkillEntry;
use systemprompt_models::services::{ComponentSource, PluginConfig};

use super::{BundleContent, BundleFile, PluginBundle};

const AUX_SUBDIRS: &[&str] = &[
    "scripts",
    "references",
    "templates",
    "diagnostics",
    "data",
    "assets",
];

const BINARY_EXTS: &[&str] = &[
    "pyc", "pyo", "so", "dll", "exe", "png", "jpg", "jpeg", "gif", "bmp", "ico", "ttf", "woff",
    "woff2", "eot", "zip", "tar", "gz", "bz2", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx",
];

pub(super) fn append_skill_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    agent_ids: &[AgentId],
    bundle: &mut PluginBundle,
) {
    let selected = resolve_skill_ids(config, content, agent_ids);
    for skill in content.skills.iter().filter(|s| selected.contains(&s.id)) {
        let kebab = skill.id.as_str().replace('_', "-");
        bundle.insert(
            format!("skills/{kebab}/SKILL.md"),
            BundleFile {
                bytes: skill_md(&kebab, skill).into_bytes(),
                executable: false,
            },
        );
        append_aux_files(&kebab, skill, bundle);
    }
}

fn resolve_skill_ids(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    agent_ids: &[AgentId],
) -> BTreeSet<SkillId> {
    let mut ids = BTreeSet::new();
    match config.skills.source {
        ComponentSource::Explicit => {
            for raw in &config.skills.include {
                insert_skill_id(&mut ids, raw);
            }
        },
        ComponentSource::Instance => {
            for skill in content.skills {
                if !config
                    .skills
                    .exclude
                    .iter()
                    .any(|ex| ex == skill.id.as_str())
                {
                    ids.insert(skill.id.clone());
                }
            }
        },
    }

    for agent in content.agents.iter().filter(|a| agent_ids.contains(&a.id)) {
        for raw in &agent.skills.include {
            insert_skill_id(&mut ids, raw);
        }
    }

    ids
}

fn insert_skill_id(ids: &mut BTreeSet<SkillId>, raw: &str) {
    match SkillId::try_new(raw) {
        Ok(id) => {
            ids.insert(id);
        },
        Err(e) => {
            tracing::warn!(error = %e, skill_id = raw, "bundle: ignoring invalid skill id");
        },
    }
}

fn skill_md(kebab: &str, skill: &SkillEntry) -> String {
    format!(
        "---\nname: {kebab}\ndescription: \"{}\"\n---\n\n{}\n",
        skill.description.replace('"', "\\\""),
        skill.instructions.trim()
    )
}

fn append_aux_files(kebab: &str, skill: &SkillEntry, bundle: &mut PluginBundle) {
    let Some(skill_dir) = Path::new(&skill.file_path).parent() else {
        return;
    };
    for subdir in AUX_SUBDIRS {
        let dir = skill_dir.join(subdir);
        if dir.is_dir() {
            collect_aux(&dir, &dir, kebab, subdir, bundle);
        }
    }
}

fn collect_aux(base: &Path, current: &Path, kebab: &str, subdir: &str, bundle: &mut PluginBundle) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if name == "__pycache__" {
                continue;
            }
            collect_aux(base, &path, kebab, subdir, bundle);
            continue;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && BINARY_EXTS.contains(&ext.to_lowercase().as_str())
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let executable = matches!(path.extension().and_then(|e| e.to_str()), Some("sh" | "py"));
        bundle.insert(
            format!("skills/{kebab}/{subdir}/{rel}"),
            BundleFile { bytes, executable },
        );
    }
}
