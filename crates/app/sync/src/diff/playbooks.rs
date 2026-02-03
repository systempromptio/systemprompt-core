use crate::models::{DiffStatus, DiskPlaybook, PlaybookDiffItem, PlaybooksDiffResult};
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use systemprompt_agent::models::Playbook;
use systemprompt_agent::repository::content::PlaybookRepository;
use systemprompt_database::DatabaseProvider;
use tracing::warn;

#[derive(Debug)]
pub struct PlaybooksDiffCalculator {
    playbook_repo: PlaybookRepository,
}

impl PlaybooksDiffCalculator {
    pub fn new(db: Arc<dyn DatabaseProvider>) -> Self {
        Self {
            playbook_repo: PlaybookRepository::new(db),
        }
    }

    pub async fn calculate_diff(&self, playbooks_path: &Path) -> Result<PlaybooksDiffResult> {
        let db_playbooks = self.playbook_repo.list_all().await?;
        let db_map: HashMap<String, Playbook> = db_playbooks
            .into_iter()
            .map(|p| (p.playbook_id.as_str().to_string(), p))
            .collect();

        let disk_playbooks = Self::scan_disk_playbooks(playbooks_path);

        let mut result = PlaybooksDiffResult::default();

        for (playbook_id, disk_playbook) in &disk_playbooks {
            let disk_hash = compute_playbook_hash(disk_playbook);

            match db_map.get(playbook_id.as_str()) {
                None => {
                    result.added.push(PlaybookDiffItem {
                        playbook_id: playbook_id.clone(),
                        file_path: disk_playbook.file_path.clone(),
                        category: disk_playbook.category.clone(),
                        domain: disk_playbook.domain.clone(),
                        status: DiffStatus::Added,
                        disk_hash: Some(disk_hash),
                        db_hash: None,
                        name: Some(disk_playbook.name.clone()),
                    });
                },
                Some(db_playbook) => {
                    let db_hash = compute_db_playbook_hash(db_playbook);
                    if db_hash == disk_hash {
                        result.unchanged += 1;
                    } else {
                        result.modified.push(PlaybookDiffItem {
                            playbook_id: playbook_id.clone(),
                            file_path: disk_playbook.file_path.clone(),
                            category: disk_playbook.category.clone(),
                            domain: disk_playbook.domain.clone(),
                            status: DiffStatus::Modified,
                            disk_hash: Some(disk_hash),
                            db_hash: Some(db_hash),
                            name: Some(disk_playbook.name.clone()),
                        });
                    }
                },
            }
        }

        for (playbook_id, db_playbook) in &db_map {
            if !disk_playbooks.contains_key(playbook_id.as_str()) {
                result.removed.push(PlaybookDiffItem {
                    playbook_id: playbook_id.clone(),
                    file_path: db_playbook.file_path.clone(),
                    category: db_playbook.category.clone(),
                    domain: db_playbook.domain.clone(),
                    status: DiffStatus::Removed,
                    disk_hash: None,
                    db_hash: Some(compute_db_playbook_hash(db_playbook)),
                    name: Some(db_playbook.name.clone()),
                });
            }
        }

        Ok(result)
    }

    fn scan_disk_playbooks(path: &Path) -> HashMap<String, DiskPlaybook> {
        use walkdir::WalkDir;

        let mut playbooks = HashMap::new();

        if !path.exists() {
            return playbooks;
        }

        for entry in WalkDir::new(path)
            .min_depth(2)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        {
            let file_path = entry.path();

            if let Ok(relative) = file_path.strip_prefix(path) {
                let components: Vec<&str> = relative
                    .components()
                    .filter_map(|c| c.as_os_str().to_str())
                    .collect();

                if components.len() >= 2 {
                    let category = components[0];
                    let filename = components[components.len() - 1];
                    let domain_name = filename.strip_suffix(".md").unwrap_or(filename);

                    let domain_parts: Vec<&str> = components[1..components.len() - 1]
                        .iter()
                        .copied()
                        .chain(std::iter::once(domain_name))
                        .collect();
                    let domain = domain_parts.join("/");

                    match parse_playbook_file(file_path, category, &domain) {
                        Ok(playbook) => {
                            playbooks.insert(playbook.playbook_id.clone(), playbook);
                        },
                        Err(e) => {
                            warn!("Failed to parse playbook at {}: {}", file_path.display(), e);
                        },
                    }
                }
            }
        }

        playbooks
    }
}

fn parse_playbook_file(md_path: &Path, category: &str, domain: &str) -> Result<DiskPlaybook> {
    let content = std::fs::read_to_string(md_path)?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!("Invalid frontmatter format"));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])?;
    let instructions = parts[2].trim().to_string();

    let playbook_id = format!("{}_{}", category, domain.replace('/', "_"));

    let name = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing title in frontmatter"))?
        .to_string();

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(DiskPlaybook {
        playbook_id,
        name,
        description,
        instructions,
        category: category.to_string(),
        domain: domain.to_string(),
        file_path: md_path.to_string_lossy().to_string(),
    })
}

fn compute_playbook_hash(playbook: &DiskPlaybook) -> String {
    let mut hasher = Sha256::new();
    hasher.update(playbook.name.as_bytes());
    hasher.update(playbook.description.as_bytes());
    hasher.update(playbook.instructions.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn compute_db_playbook_hash(playbook: &Playbook) -> String {
    let mut hasher = Sha256::new();
    hasher.update(playbook.name.as_bytes());
    hasher.update(playbook.description.as_bytes());
    hasher.update(playbook.instructions.as_bytes());
    format!("{:x}", hasher.finalize())
}
