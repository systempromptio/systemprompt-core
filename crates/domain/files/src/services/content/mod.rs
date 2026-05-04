use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, FileId};

use crate::error::FilesResult;
use crate::models::{ContentFile, File, FileRole};
use crate::repository::FileRepository;

#[derive(Debug, Clone)]
pub struct ContentService {
    repository: FileRepository,
}

impl ContentService {
    pub fn new(db: &DbPool) -> FilesResult<Self> {
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

    pub async fn link_to_content(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
        role: FileRole,
        display_order: i32,
    ) -> FilesResult<ContentFile> {
        self.repository
            .link_to_content(content_id, file_id, role, display_order)
            .await
    }

    pub async fn unlink_from_content(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
    ) -> FilesResult<()> {
        self.repository
            .unlink_from_content(content_id, file_id)
            .await
    }

    pub async fn list_files_by_content(
        &self,
        content_id: &ContentId,
    ) -> FilesResult<Vec<(File, ContentFile)>> {
        self.repository.list_files_by_content(content_id).await
    }

    pub async fn list_content_by_file(&self, file_id: &FileId) -> FilesResult<Vec<ContentFile>> {
        self.repository.list_content_by_file(file_id).await
    }

    pub async fn find_featured_image(&self, content_id: &ContentId) -> FilesResult<Option<File>> {
        self.repository.find_featured_image(content_id).await
    }

    pub async fn set_featured(&self, file_id: &FileId, content_id: &ContentId) -> FilesResult<()> {
        self.repository.set_featured(file_id, content_id).await
    }
}
