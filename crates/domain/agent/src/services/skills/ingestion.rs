use crate::models::Skill;
use crate::repository::content::SkillRepository;
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::path::Path;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SkillId, SourceId};
use systemprompt_models::IngestionReport;

const SKILL_FILENAME: &str = "SKILL.md";
const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, serde::Deserialize)]
struct SkillConfig {
    id: String,
    name: String,
    description: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    tags: Vec<String>,
}

const fn default_enabled() -> bool {
    true
}

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
                        .push(format!("{}: {}", skill_dir.display(), e));
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
        let skill_file = skill_dir.join(SKILL_FILENAME);

        if !skill_file.exists() {
            return Err(anyhow!("No {} found in skill directory", SKILL_FILENAME));
        }

        let markdown_text = std::fs::read_to_string(&skill_file)?;
        let instructions = Self::parse_skill_markdown(&markdown_text)?;

        let dir_name = skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

        let config_path = skill_dir.join(CONFIG_FILENAME);
        let (skill_id_str, name, description, enabled, tags) = if config_path.exists() {
            let config_text = std::fs::read_to_string(&config_path)?;
            let config: SkillConfig = serde_yaml::from_str(&config_text)
                .map_err(|e| anyhow!("Failed to parse {}: {}", CONFIG_FILENAME, e))?;
            (
                config.id,
                config.name,
                config.description,
                config.enabled,
                config.tags,
            )
        } else {
            let md_description = Self::extract_description(&markdown_text);
            (
                dir_name.replace('-', "_"),
                dir_name.replace('_', " "),
                md_description,
                true,
                Vec::new(),
            )
        };

        let file_path = skill_file.to_string_lossy().to_string();

        let skill = Skill {
            skill_id: SkillId::new(skill_id_str),
            file_path,
            name,
            description,
            instructions,
            enabled,
            tags,
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
                tracing::debug!(error = %err, "Failed to read directory entry, skipping");
                err
            })
            .ok()
        }) {
            if entry.file_type().is_dir() && entry.file_name() != "." {
                let skill_file = entry.path().join(SKILL_FILENAME);
                if skill_file.exists() {
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

    fn parse_skill_markdown(markdown: &str) -> Result<String> {
        let parts: Vec<&str> = markdown.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(anyhow!("Invalid frontmatter format"));
        }

        Ok(parts[2].trim().to_string())
    }

    fn extract_description(markdown: &str) -> String {
        let parts: Vec<&str> = markdown.splitn(3, "---").collect();
        if parts.len() < 3 {
            return String::new();
        }

        serde_yaml::from_str::<serde_yaml::Value>(parts[1])
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to parse skill frontmatter YAML");
                e
            })
            .ok()
            .and_then(|v| {
                v.get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(String::new)
    }
}
