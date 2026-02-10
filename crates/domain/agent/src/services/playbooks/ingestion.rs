use crate::models::Playbook;
use crate::repository::content::PlaybookRepository;
use anyhow::{anyhow, Result};
use std::path::Path;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{PlaybookId, SourceId};
use systemprompt_models::IngestionReport;

#[derive(Debug)]
pub struct PlaybookIngestionService {
    playbook_repo: PlaybookRepository,
}

impl PlaybookIngestionService {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            playbook_repo: PlaybookRepository::new(db)?,
        })
    }

    pub async fn ingest_directory(
        &self,
        path: &Path,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<IngestionReport> {
        let mut report = IngestionReport::new();

        let playbook_files = self.scan_playbook_files(path)?;
        report.files_found = playbook_files.len();

        for (file_path, category, domain) in playbook_files {
            match self
                .ingest_playbook(
                    &file_path,
                    &category,
                    &domain,
                    source_id.clone(),
                    override_existing,
                )
                .await
            {
                Ok(_) => {
                    report.files_processed += 1;
                },
                Err(e) => {
                    report
                        .errors
                        .push(format!("{}: {}", file_path.display(), e));
                },
            }
        }

        Ok(report)
    }

    async fn ingest_playbook(
        &self,
        playbook_file: &Path,
        category: &str,
        domain: &str,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<()> {
        let markdown_text = std::fs::read_to_string(playbook_file)?;
        let (metadata, instructions) = Self::parse_playbook_markdown(&markdown_text)?;

        let playbook_id = format!("{}_{}", category, domain.replace('/', "_"));

        let name = metadata
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Playbook must have 'title' in frontmatter"))?
            .to_string();

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let file_path = playbook_file.to_string_lossy().to_string();
        let enabled = metadata
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let tags = metadata
            .get("keywords")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_else(Vec::new);

        let playbook = Playbook {
            playbook_id: PlaybookId::new(playbook_id),
            file_path: file_path.clone(),
            name,
            description,
            instructions,
            enabled,
            tags,
            category: category.to_string(),
            domain: domain.to_string(),
            source_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if self
            .playbook_repo
            .get_by_playbook_id(&playbook.playbook_id)
            .await?
            .is_some()
        {
            if override_existing {
                self.playbook_repo
                    .update(&playbook.playbook_id, &playbook)
                    .await?;
            }
        } else {
            self.playbook_repo.create(&playbook).await?;
        }

        Ok(())
    }

    fn scan_playbook_files(&self, dir: &Path) -> Result<Vec<(std::path::PathBuf, String, String)>> {
        use walkdir::WalkDir;

        let mut playbook_files = Vec::new();

        for entry in WalkDir::new(dir).min_depth(2).into_iter().filter_map(|e| {
            e.map_err(|err| {
                tracing::debug!(error = %err, "Failed to read directory entry, skipping");
                err
            })
            .ok()
        }) {
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "md" {
                        if let Ok(relative) = path.strip_prefix(dir) {
                            let components: Vec<&str> = relative
                                .components()
                                .filter_map(|c| c.as_os_str().to_str())
                                .collect();

                            if components.len() >= 2 {
                                let category = components[0].to_string();
                                let filename = components[components.len() - 1];
                                let domain_name = filename.strip_suffix(".md").unwrap_or(filename);

                                let domain_parts: Vec<&str> = components[1..components.len() - 1]
                                    .iter()
                                    .copied()
                                    .chain(std::iter::once(domain_name))
                                    .collect();
                                let domain = domain_parts.join("/");

                                playbook_files.push((path.to_path_buf(), category, domain));
                            }
                        }
                    }
                }
            }
        }

        Ok(playbook_files)
    }

    fn parse_playbook_markdown(markdown: &str) -> Result<(serde_yaml::Mapping, String)> {
        let parts: Vec<&str> = markdown.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(anyhow!("Invalid frontmatter format"));
        }

        let metadata = serde_yaml::from_str::<serde_yaml::Value>(parts[1])?
            .as_mapping()
            .ok_or_else(|| anyhow!("Invalid YAML in frontmatter"))?
            .clone();

        let instructions = parts[2].trim().to_string();

        Ok((metadata, instructions))
    }
}
