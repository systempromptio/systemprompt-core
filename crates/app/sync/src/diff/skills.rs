use super::{compute_db_skill_hash, compute_skill_hash};
use crate::models::{DiffStatus, DiskSkill, SkillDiffItem, SkillsDiffResult};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::Path;
use systemprompt_agent::models::Skill;
use systemprompt_agent::repository::content::SkillRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SkillId;
use systemprompt_models::{DiskSkillConfig, SKILL_CONFIG_FILENAME, strip_frontmatter};
use tracing::warn;

#[derive(Debug)]
pub struct SkillsDiffCalculator {
    skill_repo: SkillRepository,
}

impl SkillsDiffCalculator {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            skill_repo: SkillRepository::new(db)?,
        })
    }

    pub async fn calculate_diff(&self, skills_path: &Path) -> Result<SkillsDiffResult> {
        let db_skills = self.skill_repo.list_all().await?;
        let db_map: HashMap<SkillId, Skill> =
            db_skills.into_iter().map(|s| (s.id.clone(), s)).collect();

        let disk_skills = Self::scan_disk_skills(skills_path)?;

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
                    if db_hash == disk_hash {
                        result.unchanged += 1;
                    } else {
                        result.modified.push(SkillDiffItem {
                            skill_id: skill_id.clone(),
                            file_path: disk_skill.file_path.clone(),
                            status: DiffStatus::Modified,
                            disk_hash: Some(disk_hash),
                            db_hash: Some(db_hash),
                            name: Some(disk_skill.name.clone()),
                        });
                    }
                },
            }
        }

        for (skill_id, db_skill) in &db_map {
            if !disk_skills.contains_key(skill_id) {
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

    fn scan_disk_skills(path: &Path) -> Result<HashMap<SkillId, DiskSkill>> {
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

            let config_path = skill_path.join(SKILL_CONFIG_FILENAME);
            if !config_path.exists() {
                continue;
            }

            match parse_skill_file(&config_path, &skill_path) {
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

fn parse_skill_file(config_path: &Path, skill_dir: &Path) -> Result<DiskSkill> {
    let config_text = std::fs::read_to_string(config_path)?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text)?;

    let dir_name = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

    let content_path = skill_dir.join(config.content_file());

    let skill_id_str = if config.id.is_empty() {
        dir_name.replace('-', "_")
    } else {
        config.id
    };
    let skill_id = SkillId::new(skill_id_str);

    let name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name
    };

    let instructions = if content_path.exists() {
        let raw = std::fs::read_to_string(&content_path)?;
        strip_frontmatter(&raw)
    } else {
        String::new()
    };

    Ok(DiskSkill {
        skill_id,
        name,
        description: config.description,
        instructions,
        file_path: content_path.to_string_lossy().to_string(),
    })
}
