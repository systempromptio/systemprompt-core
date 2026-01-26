use crate::repository::content::PlaybookRepository;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::PlaybookId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookMetadata {
    pub playbook_id: PlaybookId,
    pub name: String,
    pub category: String,
    pub domain: String,
}

#[derive(Clone)]
pub struct PlaybookService {
    playbook_repo: Arc<PlaybookRepository>,
}

impl std::fmt::Debug for PlaybookService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaybookService")
            .field("playbook_repo", &"<PlaybookRepository>")
            .finish()
    }
}

impl PlaybookService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            playbook_repo: Arc::new(PlaybookRepository::new(db_pool)),
        }
    }

    pub async fn load_playbook(&self, playbook_id: &str) -> Result<String> {
        let playbook_id_typed = PlaybookId::new(playbook_id);

        let playbook = self
            .playbook_repo
            .get_by_playbook_id(&playbook_id_typed)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Playbook not found in database: {} (ensure playbook is synced via \
                     PlaybookIngestionService)",
                    playbook_id
                )
            })?;

        tracing::info!(playbook_id = %playbook.playbook_id, "Loaded playbook");

        Ok(playbook.instructions)
    }

    pub async fn list_playbook_ids(&self) -> Result<Vec<String>> {
        let playbooks = self.playbook_repo.list_enabled().await?;
        Ok(playbooks
            .into_iter()
            .map(|p| p.playbook_id.to_string())
            .collect())
    }

    pub async fn load_playbook_metadata(&self, playbook_id: &str) -> Result<PlaybookMetadata> {
        let playbook_id_typed = PlaybookId::new(playbook_id);

        let playbook = self
            .playbook_repo
            .get_by_playbook_id(&playbook_id_typed)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Playbook not found in database: {} (ensure playbook is synced via \
                     PlaybookIngestionService)",
                    playbook_id
                )
            })?;

        tracing::info!(playbook_id = %playbook.playbook_id, "Loaded playbook metadata");

        Ok(PlaybookMetadata {
            playbook_id: playbook.playbook_id,
            name: playbook.name,
            category: playbook.category,
            domain: playbook.domain,
        })
    }

    pub async fn list_by_category(&self, category: &str) -> Result<Vec<PlaybookMetadata>> {
        let playbooks = self.playbook_repo.list_by_category(category).await?;
        Ok(playbooks
            .into_iter()
            .map(|p| PlaybookMetadata {
                playbook_id: p.playbook_id,
                name: p.name,
                category: p.category,
                domain: p.domain,
            })
            .collect())
    }
}
