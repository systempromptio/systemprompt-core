//! S3-compatible [`FileStorage`] backend (AWS S3, `MinIO`, Ceph RGW, R2, …).
//!
//! Objects are named `{prefix}/{id}` in one bucket, the same naming the
//! Cloud Storage backend uses. Requests are signed with AWS Signature
//! Version 4 from static credentials; `path_style` addresses the bucket as
//! `{endpoint}/{bucket}/{key}` instead of `{bucket}.{endpoint host}/{key}`,
//! which most self-hosted S3 implementations require.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod error;
mod sigv4;

use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;
use systemprompt_traits::{
    ByteRange, FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
    StoredFileRead,
};
use url::Url;

pub use error::S3Error;

use crate::object_id::{id_for, mime_for, relative_path};
use crate::remote_read::RemoteRead;
use client::Lookup;

pub const DEFAULT_S3_REGION: &str = "us-east-1";

/// Static S3 credentials, as held in the secrets store.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct S3Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub session_token: Option<String>,
}

impl std::fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Credentials")
            .field("access_key_id", &self.access_key_id)
            .finish_non_exhaustive()
    }
}

/// Bucket, prefix, region and endpoint for an [`S3FileStorage`].
#[derive(Debug, Clone)]
pub struct S3Params {
    pub bucket: String,
    pub prefix: Option<String>,
    pub region: String,
    pub endpoint: Url,
    pub path_style: bool,
    pub public_read: bool,
}

impl S3Params {
    pub fn new(
        bucket: impl Into<String>,
        region: Option<String>,
        endpoint: Option<&str>,
    ) -> Result<Self, url::ParseError> {
        let region = region.unwrap_or_else(|| DEFAULT_S3_REGION.to_owned());
        let endpoint = match endpoint {
            Some(endpoint) => Url::parse(endpoint)?,
            None => Url::parse(&format!("https://s3.{region}.amazonaws.com/"))?,
        };
        Ok(Self {
            bucket: bucket.into(),
            prefix: None,
            region,
            endpoint,
            path_style: false,
            public_read: false,
        })
    }

    #[must_use]
    pub fn with_prefix(mut self, prefix: Option<String>) -> Self {
        self.prefix = prefix;
        self
    }

    #[must_use]
    pub const fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    #[must_use]
    pub const fn with_public_read(mut self, public_read: bool) -> Self {
        self.public_read = public_read;
        self
    }
}

/// Files stored as objects in one S3-compatible bucket.
pub struct S3FileStorage {
    params: S3Params,
    credentials: S3Credentials,
    http: reqwest::Client,
}

impl std::fmt::Debug for S3FileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3FileStorage")
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl S3FileStorage {
    #[must_use]
    pub const fn new(params: S3Params, credentials: S3Credentials, http: reqwest::Client) -> Self {
        Self {
            params,
            credentials,
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
impl FileStorage for S3FileStorage {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        let id = id_for(relative_path(path)?);
        self.put(&self.name_for(&id), mime_for(path), content)
            .await
            .map_err(|e| e.into_storage(&id))?;
        Ok(id)
    }

    async fn open(
        &self,
        id: &StoredFileId,
        range: Option<ByteRange>,
    ) -> FileStorageResult<StoredFileRead> {
        let name = self.object_name(id)?;
        let response = self
            .get(&name, range)
            .await
            .map_err(|e| e.into_storage(id))?;
        match crate::remote_read::into_read(id, response) {
            RemoteRead::Read(read) => Ok(read),
            RemoteRead::Unsatisfiable(Some(size)) => {
                Err(FileStorageError::RangeNotSatisfiable { size })
            },
            RemoteRead::Unsatisfiable(None) => Err(FileStorageError::RangeNotSatisfiable {
                size: crate::remote_read::stat_size(&self.metadata(id).await?),
            }),
        }
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        let name = self.object_name(id)?;
        self.remove(&name).await.map_err(|e| e.into_storage(id))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        let name = self.object_name(id)?;
        let head = match self.head(&name).await.map_err(|e| e.into_storage(id))? {
            Lookup::Found(head) => head,
            Lookup::Missing => return Err(FileStorageError::NotFound(id.as_str().to_owned())),
        };
        Ok(StoredFileMetadata {
            id: id.clone(),
            path: id.as_str().to_owned(),
            mime_type: head
                .content_type
                .unwrap_or_else(|| mime_for(Path::new(id.as_str())).to_owned()),
            size_bytes: head.size_bytes,
            etag: head.etag,
            created_at: head.last_modified,
            updated_at: head.last_modified,
        })
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        let name = self.object_name(id)?;
        match self.head(&name).await.map_err(|e| e.into_storage(id))? {
            Lookup::Found(_) => Ok(true),
            Lookup::Missing => Ok(false),
        }
    }

    fn public_url(&self, id: &StoredFileId) -> Option<String> {
        if !self.params.public_read {
            return None;
        }
        let name = self.object_name(id).ok()?;
        self.object_url(&name).ok().map(|(url, _)| url.to_string())
    }
}
