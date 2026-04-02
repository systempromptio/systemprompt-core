use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use systemprompt_traits::{FileStorage, StoredFileId, StoredFileMetadata};
use tokio::sync::Mutex;

pub struct MockFileStorage {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    metadata: Arc<Mutex<HashMap<String, StoredFileMetadata>>>,
    store_error: Option<String>,
}

impl MockFileStorage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            metadata: Arc::new(Mutex::new(HashMap::new())),
            store_error: None,
        }
    }

    #[must_use]
    pub fn with_file(self, id: impl Into<String>, bytes: Vec<u8>) -> Self {
        let id_str = id.into();
        let file_id = StoredFileId::new(id_str.clone());
        let meta = StoredFileMetadata {
            id: file_id,
            path: format!("mock/{id_str}"),
            mime_type: "application/octet-stream".to_string(),
            size_bytes: Some(bytes.len() as i64),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let files = self.files.clone();
        let metadata = self.metadata.clone();

        let files_guard = files.blocking_lock();
        let mut files_map = files_guard;
        files_map.insert(id_str.clone(), bytes);
        drop(files_map);

        let metadata_guard = metadata.blocking_lock();
        let mut meta_map = metadata_guard;
        meta_map.insert(id_str, meta);
        drop(meta_map);

        self
    }

    #[must_use]
    pub fn with_store_error(mut self, err: impl Into<String>) -> Self {
        self.store_error = Some(err.into());
        self
    }

    pub async fn stored_files(&self) -> HashMap<String, Vec<u8>> {
        self.files.lock().await.clone()
    }
}

impl Default for MockFileStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileStorage for MockFileStorage {
    async fn store(&self, path: &Path, content: &[u8]) -> Result<StoredFileId> {
        if let Some(ref err) = self.store_error {
            return Err(anyhow!("{err}"));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let file_id = StoredFileId::new(id.clone());

        let meta = StoredFileMetadata {
            id: file_id.clone(),
            path: path.to_string_lossy().to_string(),
            mime_type: "application/octet-stream".to_string(),
            size_bytes: Some(content.len() as i64),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.files.lock().await.insert(id.clone(), content.to_vec());
        self.metadata.lock().await.insert(id, meta);

        Ok(file_id)
    }

    async fn retrieve(&self, id: &StoredFileId) -> Result<Vec<u8>> {
        self.files
            .lock()
            .await
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| anyhow!("File not found: {id}"))
    }

    async fn delete(&self, id: &StoredFileId) -> Result<()> {
        self.files.lock().await.remove(id.as_str());
        self.metadata.lock().await.remove(id.as_str());
        Ok(())
    }

    async fn metadata(&self, id: &StoredFileId) -> Result<StoredFileMetadata> {
        self.metadata
            .lock()
            .await
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| anyhow!("File not found: {id}"))
    }

    async fn exists(&self, id: &StoredFileId) -> Result<bool> {
        Ok(self.files.lock().await.contains_key(id.as_str()))
    }
}
