use super::super::hash::safe_id_segment;
use crate::manifest::SkillEntry;
use crate::paths;
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
            id: &s.id,
            name: &s.name,
            description: &s.description,
            file_path: &s.file_path,
            tags: &s.tags,
            sha256: &s.sha256,
        }
    }
}

pub fn write_skills(meta_dir: &Path, skills: &[SkillEntry]) -> Result<(), String> {
    let dir = meta_dir.join(paths::SKILLS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("clear skills dir: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create skills dir: {e}"))?;

    write_index(&dir, skills)?;
    for skill in skills {
        write_one_skill(&dir, skill)?;
    }
    Ok(())
}

fn write_index(dir: &Path, skills: &[SkillEntry]) -> Result<(), String> {
    let index: Vec<SkillIndexEntry<'_>> = skills.iter().map(SkillIndexEntry::from).collect();
    let index_path = dir.join("index.json");
    let bytes =
        serde_json::to_vec_pretty(&index).map_err(|e| format!("serialize skills index: {e}"))?;
    fs::write(&index_path, bytes).map_err(|e| format!("write {}: {e}", index_path.display()))
}

fn write_one_skill(dir: &Path, skill: &SkillEntry) -> Result<(), String> {
    if !safe_id_segment(&skill.id) {
        return Err(format!("manifest contained unsafe skill id: {}", skill.id));
    }
    let skill_dir = dir.join(&skill.id);
    fs::create_dir_all(&skill_dir).map_err(|e| format!("create {}: {e}", skill_dir.display()))?;
    let meta = SkillIndexEntry::from(skill);
    let meta_bytes = serde_json::to_vec_pretty(&meta)
        .map_err(|e| format!("serialize skill metadata for {}: {e}", skill.id))?;
    fs::write(skill_dir.join("metadata.json"), meta_bytes)
        .map_err(|e| format!("write skill metadata for {}: {e}", skill.id))?;
    fs::write(skill_dir.join("SKILL.md"), &skill.instructions)
        .map_err(|e| format!("write SKILL.md for {}: {e}", skill.id))?;
    Ok(())
}
