use systemprompt_database::DbPool;
use systemprompt_identifiers::{FileId, UserId};

use crate::error::FilesResult;
use crate::models::{File, FileMetadata};
use crate::repository::{FileRepository, FileStats, InsertFileRequest};

#[derive(Debug, Clone)]
pub struct FileService {
    repository: FileRepository,
}

impl FileService {
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

    pub async fn insert(&self, request: InsertFileRequest) -> FilesResult<FileId> {
        self.repository.insert(request).await
    }

    pub async fn insert_file(&self, file: &File) -> FilesResult<FileId> {
        self.repository.insert_file(file).await
    }

    pub async fn find_by_id(&self, id: &FileId) -> FilesResult<Option<File>> {
        self.repository.find_by_id(id).await
    }

    pub async fn find_by_path(&self, path: &str) -> FilesResult<Option<File>> {
        self.repository.find_by_path(path).await
    }

    pub async fn list_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> FilesResult<Vec<File>> {
        self.repository.list_by_user(user_id, limit, offset).await
    }

    pub async fn list_all(&self, limit: i64, offset: i64) -> FilesResult<Vec<File>> {
        self.repository.list_all(limit, offset).await
    }

    pub async fn delete(&self, id: &FileId) -> FilesResult<()> {
        self.repository.delete(id).await
    }

    pub async fn update_metadata(&self, id: &FileId, metadata: &FileMetadata) -> FilesResult<()> {
        self.repository.update_metadata(id, metadata).await
    }

    pub async fn get_stats(&self) -> FilesResult<FileStats> {
        self.repository.get_stats().await
    }

    pub async fn search_by_path(&self, query: &str, limit: i64) -> FilesResult<Vec<File>> {
        self.repository.search_by_path(query, limit).await
    }
}
