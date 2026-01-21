use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::models::File;
use crate::repository::FileRepository;

#[derive(Debug, Clone)]
pub struct AiService {
    repository: FileRepository,
}

impl AiService {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            repository: FileRepository::new(db)?,
        })
    }

    pub const fn from_repository(repository: FileRepository) -> Self {
        Self { repository }
    }

    pub const fn repository(&self) -> &FileRepository {
        &self.repository
    }

    pub async fn list_ai_images(&self, limit: i64, offset: i64) -> Result<Vec<File>> {
        self.repository.list_ai_images(limit, offset).await
    }

    pub async fn list_ai_images_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<File>> {
        self.repository
            .list_ai_images_by_user(user_id, limit, offset)
            .await
    }

    pub async fn count_ai_images_by_user(&self, user_id: &UserId) -> Result<i64> {
        self.repository.count_ai_images_by_user(user_id).await
    }

    pub async fn count_ai_images(&self) -> Result<i64> {
        self.repository.count_ai_images().await
    }
}
