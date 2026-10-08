//! Google Cloud Storage [`FileStorage`] backend over the JSON API.
//!
//! Objects are named `{prefix}/{id}` in one bucket, where `id` is the same
//! root-relative storage id [`crate::LocalFileStorage`] uses, so a domain
//! crate cannot tell the backends apart. Authentication is an injected
//! [`GcsTokenSource`]; the API and upload endpoints are parameters, so tests
//! point them at a local server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod error;
mod token;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_traits::{
    FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
};
use url::Url;

pub use error::GcsError;
pub use token::{DEFAULT_METADATA_TOKEN_URL, GcsTokenSource, MetadataServerTokens};

use crate::object_id::{id_for, mime_for, relative_path};
use client::Lookup;

pub const DEFAULT_GCS_ENDPOINT: &str = "https://storage.googleapis.com/";

/// Bucket, object prefix and endpoints for a [`GcsFileStorage`].
#[derive(Debug, Clone)]
pub struct GcsParams {
    pub bucket: String,
    pub prefix: Option<String>,
    pub public_read: bool,
    pub api_base: Url,
    pub upload_base: Url,
}

impl GcsParams {
    pub fn new(
        bucket: impl Into<String>,
        prefix: Option<String>,
        public_read: bool,
    ) -> Result<Self, url::ParseError> {
        let endpoint = Url::parse(DEFAULT_GCS_ENDPOINT)?;
        Ok(Self {
            bucket: bucket.into(),
            prefix,
            public_read,
            api_base: endpoint.clone(),
            upload_base: endpoint,
        })
    }

    #[must_use]
    pub fn with_endpoints(mut self, api_base: Url, upload_base: Url) -> Self {
        self.api_base = api_base;
        self.upload_base = upload_base;
        self
    }
}

/// Files stored as objects in one Cloud Storage bucket.
pub struct GcsFileStorage {
    params: GcsParams,
    tokens: Arc<dyn GcsTokenSource>,
    http: reqwest::Client,
}

impl std::fmt::Debug for GcsFileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsFileStorage")
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl GcsFileStorage {
    #[must_use]
    pub fn new(params: GcsParams, tokens: Arc<dyn GcsTokenSource>, http: reqwest::Client) -> Self {
        Self {
            params,
            tokens,
            http,
        }
    }

    pub fn object_name(&self, id: &StoredFileId) -> Result<String, FileStorageError> {
        let relative = relative_path(Path::new(id.as_str()))?;
        Ok(self.name_for(&id_for(relative)))
    }

    fn name_for(&self, id: &StoredFileId) -> String {
        self.params.prefix.as_deref().map_or_else(
            || id.as_str().to_owned(),
            |prefix| format!("{prefix}/{}", id.as_str()),
        )
    }
}

#[async_trait]
impl FileStorage for GcsFileStorage {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        let id = id_for(relative_path(path)?);
        self.upload(&self.name_for(&id), mime_for(path), content)
            .await
            .map_err(|e| e.into_storage(&id))?;
        Ok(id)
    }

    async fn retrieve(&self, id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
        let name = self.object_name(id)?;
        self.download(&name).await.map_err(|e| e.into_storage(id))
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        let name = self.object_name(id)?;
        self.remove(&name).await.map_err(|e| e.into_storage(id))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let name = self.object_name(id)?;
        let object = match self.resource(&name).await.map_err(|e| e.into_storage(id))? {
            Lookup::Found(object) => object,
            Lookup::Missing => return Err(FileStorageError::NotFound(id.as_str().to_owned())),
        };
        let size_bytes = object
            .size
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|source| {
                FileStorageError::Backend(Box::new(GcsError::ObjectSize {
                    name: name.clone(),
                    source,
                }))
            })?;
        let created_at = object.time_created.ok_or_else(|| {
            FileStorageError::Backend(Box::new(GcsError::ObjectField {
                name: name.clone(),
                field: "timeCreated",
            }))
        })?;
        Ok(StoredFileMetadata {
            id: id.clone(),
            path: id.as_str().to_owned(),
            mime_type: object
                .content_type
                .unwrap_or_else(|| mime_for(Path::new(id.as_str())).to_owned()),
            size_bytes,
            created_at,
            updated_at: object.updated.unwrap_or(created_at),
        })
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        let name = self.object_name(id)?;
        match self.resource(&name).await.map_err(|e| e.into_storage(id))? {
            Lookup::Found(_) => Ok(true),
            Lookup::Missing => Ok(false),
        }
    }

    fn public_url(&self, id: &StoredFileId) -> Option<String> {
        if !self.params.public_read {
            return None;
        }
        let name = self.object_name(id).ok()?;
        let encoded: Vec<String> = name
            .split('/')
            .map(|segment| urlencoding::encode(segment).into_owned())
            .collect();
        let mut base = self.params.api_base.to_string();
        if !base.ends_with('/') {
            base.push('/');
        }
        Some(format!(
            "{base}{}/{}",
            urlencoding::encode(&self.params.bucket),
            encoded.join("/")
        ))
    }
}
