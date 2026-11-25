//! The content–file link contract: which files a content item carries, and in
//! which role.
//!
//! The `files` member owns the links (`content_files`) and provides them as
//! [`DynContentFileLinks`], open as its file repository is: a link names a
//! content item and a file by id and carries no caller authority. The
//! `content files` commands read it; a composition without the `files`
//! member provides none, and those commands then refuse with the missing
//! dependency named.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_traits::BoxedSource;

/// The role a file plays on a content item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum ContentFileRole {
    Featured,
    Attachment,
    Inline,
    OgImage,
    Thumbnail,
}

impl ContentFileRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Featured => "featured",
            Self::Attachment => "attachment",
            Self::Inline => "inline",
            Self::OgImage => "og_image",
            Self::Thumbnail => "thumbnail",
        }
    }
}

/// A file linked to a content item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedFile {
    pub file_id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub created_at: DateTime<Utc>,
    pub role: String,
    pub display_order: i32,
}

/// A content item a file is linked to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContentLink {
    pub content_id: ContentId,
    pub role: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
#[error("the content file links failed: {0}")]
pub struct ContentFileLinksError(#[source] pub BoxedSource);

/// The links between content items and files.
///
/// `#[async_trait]` because the links are held as `Arc<dyn
/// ContentFileLinks>`.
#[async_trait]
pub trait ContentFileLinks: Send + Sync {
    async fn link(
        &self,
        content: &ContentId,
        file: &FileId,
        role: ContentFileRole,
        display_order: i32,
    ) -> Result<(), ContentFileLinksError>;

    async fn unlink(&self, content: &ContentId, file: &FileId)
    -> Result<(), ContentFileLinksError>;

    async fn files_of(&self, content: &ContentId)
    -> Result<Vec<LinkedFile>, ContentFileLinksError>;

    async fn contents_of(
        &self,
        file: &FileId,
    ) -> Result<Vec<FileContentLink>, ContentFileLinksError>;

    async fn featured(
        &self,
        content: &ContentId,
    ) -> Result<Option<LinkedFile>, ContentFileLinksError>;

    async fn set_featured(
        &self,
        content: &ContentId,
        file: &FileId,
    ) -> Result<(), ContentFileLinksError>;
}

pub type DynContentFileLinks = Arc<dyn ContentFileLinks>;
