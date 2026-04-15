use crate::models::Skill;
use crate::repository::content::SkillRepository;
use anyhow::{Result, anyhow};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SkillId, SourceId};
use systemprompt_models::{IngestionReport, SkillConfig, SkillsConfig};

#[derive(Debug)]
pub struct SkillIngestionService {
    skill_repo: SkillRepository,
}

impl SkillIngestionService {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            skill_repo: SkillRepository::new(db)?,
        })
    }

    pub async fn ingest_config(
        &self,
        config: &SkillsConfig,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<IngestionReport> {
        let mut report = IngestionReport::new();
        report.files_found = config.skills.len();

        for (key, skill_config) in &config.skills {
            match self
                .ingest_skill(key, skill_config, source_id.clone(), override_existing)
                .await
            {
                Ok(()) => {
                    report.files_processed += 1;
                },
                Err(e) => {
                    report.errors.push(format!("{key}: {e:?}"));
                },
            }
        }

        Ok(report)
    }

    async fn ingest_skill(
        &self,
        key: &str,
        config: &SkillConfig,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<()> {
        let skill_id_str = if config.id.as_str().is_empty() {
            key.to_string()
        } else {
            config.id.as_str().to_string()
        };

        let instructions = match config.instructions.as_ref() {
            Some(includable) => includable
                .as_inline()
                .ok_or_else(|| {
                    anyhow!(
                        "Skill '{}' has unresolved !include for instructions; loader must resolve \
                         includes before ingestion",
                        skill_id_str
                    )
                })?
                .to_string(),
            None => String::new(),
        };

        let skill = Skill {
            id: SkillId::new(skill_id_str),
            file_path: String::new(),
            name: config.name.clone(),
            description: config.description.clone(),
            instructions,
            enabled: config.enabled,
            tags: config.tags.clone(),
            category_id: None,
            source_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if self.skill_repo.get_by_skill_id(&skill.id).await?.is_some() {
            if override_existing {
                self.skill_repo.update(&skill.id, &skill).await?;
            }
        } else {
            self.skill_repo.create(&skill).await?;
        }

        Ok(())
    }
}
