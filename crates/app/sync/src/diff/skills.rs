use super::{compute_db_skill_hash, compute_skill_hash};
use crate::models::{DiffStatus, DiskSkill, SkillDiffItem, SkillsDiffResult};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use systemprompt_agent::models::Skill;
use systemprompt_agent::repository::content::SkillRepository;
use systemprompt_database::DatabaseProvider;
use tracing::warn;

#[derive(Debug)]
pub struct SkillsDiffCalculator {
    skill_repo: SkillRepository,
}

impl SkillsDiffCalculator {
    pub fn new(db: Arc<dyn DatabaseProvider>) -> Self {
        Self {
            skill_repo: SkillRepository::new(db),
        }
    }

    pub async fn calculate_diff(&self, skills_path: &Path) -> Result<SkillsDiffResult> {
        let db_skills = self.skill_repo.list_all().await?;
        let db_map: HashMap<String, Skill> = db_skills
            .into_iter()
            .map(|s| (s.skill_id.as_str().to_string(), s))
            .collect();

        let disk_skills = self.scan_disk_skills(skills_path)?;

        let mut result = SkillsDiffResult::default();

        for (skill_id, disk_skill) in &disk_skills {
            let disk_hash = compute_skill_hash(disk_skill);

            match db_map.get(skill_id) {
                None => {
                    result.added.push(SkillDiffItem {
                        skill_id: skill_id.clone(),
                        file_path: disk_skill.file_path.clone(),
                        status: DiffStatus::Added,
                        disk_hash: Some(disk_hash),
                        db_hash: None,
                        name: Some(disk_skill.name.clone()),
                    });
                },
                Some(db_skill) => {
                    let db_hash = compute_db_skill_hash(db_skill);
                    if db_hash != disk_hash {
                        result.modified.push(SkillDiffItem {
                            skill_id: skill_id.clone(),
                            file_path: disk_skill.file_path.clone(),
                            status: DiffStatus::Modified,
                            disk_hash: Some(disk_hash),
                            db_hash: Some(db_hash),
                            name: Some(disk_skill.name.clone()),
                        });
                    } else {
                        result.unchanged += 1;
                    }
                },
            }
        }

        for (skill_id, db_skill) in &db_map {
            if !disk_skills.contains_key(skill_id.as_str()) {
                result.removed.push(SkillDiffItem {
                    skill_id: skill_id.clone(),
                    file_path: db_skill.file_path.clone(),
                    status: DiffStatus::Removed,
                    disk_hash: None,
                    db_hash: Some(compute_db_skill_hash(db_skill)),
                    name: Some(db_skill.name.clone()),
                });
            }
        }

        Ok(result)
    }

    fn scan_disk_skills(&self, path: &Path) -> Result<HashMap<String, DiskSkill>> {
        let mut skills = HashMap::new();

        if !path.exists() {
            return Ok(skills);
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let skill_path = entry.path();

            if !skill_path.is_dir() {
                continue;
            }

            let index_path = skill_path.join("index.md");
            let skill_md_path = skill_path.join("SKILL.md");

            let md_path = if index_path.exists() {
                index_path
            } else if skill_md_path.exists() {
                skill_md_path
            } else {
                continue;
            };

            match parse_skill_file(&md_path, &skill_path) {
                Ok(skill) => {
                    skills.insert(skill.skill_id.clone(), skill);
                },
                Err(e) => {
                    warn!("Failed to parse skill at {}: {}", skill_path.display(), e);
                },
            }
        }

        Ok(skills)
    }
}

fn parse_skill_file(md_path: &Path, skill_dir: &Path) -> Result<DiskSkill> {
    let content = std::fs::read_to_string(md_path)?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!("Invalid frontmatter format"));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])?;
    let instructions = parts[2].trim().to_string();

    let dir_name = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

    let skill_id = dir_name.replace('-', "_");

    let name = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing title in frontmatter"))?
        .to_string();

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing description in frontmatter"))?
        .to_string();

    Ok(DiskSkill {
        skill_id,
        name,
        description,
        instructions,
        file_path: md_path.to_string_lossy().to_string(),
    })
}
