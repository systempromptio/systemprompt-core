use crate::models::Skill;
use crate::repository::content::SkillRepository;
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::path::Path;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SkillId, SourceId};
use systemprompt_models::{
    DiskSkillConfig, IngestionReport, SKILL_CONFIG_FILENAME, strip_frontmatter,
};

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

    pub async fn ingest_directory(
        &self,
        path: &Path,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<IngestionReport> {
        let mut report = IngestionReport::new();

        let skill_dirs = self.scan_skill_directories(path)?;
        report.files_found = skill_dirs.len();

        for skill_dir in skill_dirs {
            match self
                .ingest_skill(&skill_dir, source_id.clone(), override_existing)
                .await
            {
                Ok(_) => {
                    report.files_processed += 1;
                },
                Err(e) => {
                    report
                        .errors
                        .push(format!("{}: {:?}", skill_dir.display(), e));
                },
            }
        }

        Ok(report)
    }

    async fn ingest_skill(
        &self,
        skill_dir: &Path,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<()> {
        let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

        if !config_path.exists() {
            return Err(anyhow!(
                "No {} found in skill directory",
                SKILL_CONFIG_FILENAME
            ));
        }

        let dir_name = skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

        let config_text = std::fs::read_to_string(&config_path)?;
        let config: DiskSkillConfig = serde_yaml::from_str(&config_text)
            .map_err(|e| anyhow!("Failed to parse {}: {}", SKILL_CONFIG_FILENAME, e))?;

        let content_path = skill_dir.join(config.content_file());

        let skill_id_str = if config.id.is_empty() {
            dir_name.replace('-', "_")
        } else {
            config.id
        };

        let instructions = if content_path.exists() {
            let raw = std::fs::read_to_string(&content_path)?;
            strip_frontmatter(&raw)
        } else {
            return Err(anyhow!(
                "Content file '{}' not found",
                content_path.display()
            ));
        };

        let file_path = content_path.to_string_lossy().to_string();

        let skill = Skill {
            skill_id: SkillId::new(skill_id_str),
            file_path,
            name: config.name,
            description: config.description,
            instructions,
            enabled: config.enabled,
            tags: config.tags,
            category_id: None,
            source_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if self
            .skill_repo
            .get_by_skill_id(&skill.skill_id)
            .await?
            .is_some()
        {
            if override_existing {
                self.skill_repo.update(&skill.skill_id, &skill).await?;
            }
        } else {
            self.skill_repo.create(&skill).await?;
        }

        Ok(())
    }

    fn scan_skill_directories(&self, dir: &Path) -> Result<Vec<std::path::PathBuf>> {
        use walkdir::WalkDir;

        let mut skill_dirs = Vec::new();
        let mut seen = HashSet::new();

        for entry in WalkDir::new(dir).max_depth(2).into_iter().filter_map(|e| {
            e.map_err(|err| {
                tracing::warn!(error = %err, "Skipping unreadable directory entry during skill scan");
                err
            })
            .ok()
        }) {
            if entry.file_type().is_dir() && entry.file_name() != "." {
                let config_file = entry.path().join(SKILL_CONFIG_FILENAME);
                if config_file.exists() {
                    let path = entry.path().to_path_buf();
                    if !seen.contains(&path) {
                        skill_dirs.push(path.clone());
                        seen.insert(path);
                    }
                }
            }
        }

        Ok(skill_dirs)
    }
}
