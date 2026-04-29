use super::super::hash::safe_id_segment;
use crate::config::paths;
use crate::gateway::manifest::SkillEntry;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct SkillIndexEntry<'a> {
    id: &'a str,
    name: &'a str,
    description: &'a str,
    file_path: &'a str,
    tags: &'a [String],
    sha256: &'a str,
}

impl<'a> From<&'a SkillEntry> for SkillIndexEntry<'a> {
    fn from(s: &'a SkillEntry) -> Self {
        Self {
            id: s.id.as_str(),
            name: s.name.as_str(),
            description: &s.description,
            file_path: &s.file_path,
            tags: &s.tags,
            sha256: s.sha256.as_str(),
        }
    }
}

pub fn write_skills(meta_dir: &Path, skills: &[SkillEntry]) -> Result<(), super::ApplyError> {
    let dir = meta_dir.join(paths::SKILLS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| super::ApplyError::Io {
            context: "clear skills dir".into(),
            source: e,
        })?;
    }
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: "create skills dir".into(),
        source: e,
    })?;

    write_index(&dir, skills)?;
    for skill in skills {
        write_one_skill(&dir, skill)?;
    }
    Ok(())
}

fn write_index(dir: &Path, skills: &[SkillEntry]) -> Result<(), super::ApplyError> {
    let index: Vec<SkillIndexEntry<'_>> = skills.iter().map(SkillIndexEntry::from).collect();
    let index_path = dir.join("index.json");
    let bytes = serde_json::to_vec_pretty(&index).map_err(|e| super::ApplyError::Serialize {
        what: "skills index".into(),
        source: e,
    })?;
    fs::write(&index_path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", index_path.display()),
        source: e,
    })
}

fn write_one_skill(dir: &Path, skill: &SkillEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(super::ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let skill_dir = dir.join(skill.id.as_str());
    fs::create_dir_all(&skill_dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", skill_dir.display()),
        source: e,
    })?;
    let meta = SkillIndexEntry::from(skill);
    let meta_bytes =
        serde_json::to_vec_pretty(&meta).map_err(|e| super::ApplyError::Serialize {
            what: format!("skill metadata for {}", skill.id),
            source: e,
        })?;
    fs::write(skill_dir.join("metadata.json"), meta_bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write skill metadata for {}", skill.id),
        source: e,
    })?;
    fs::write(skill_dir.join("SKILL.md"), &skill.instructions).map_err(|e| {
        super::ApplyError::Io {
            context: format!("write SKILL.md for {}", skill.id),
            source: e,
        }
    })?;
    Ok(())
}
