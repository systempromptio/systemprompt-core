//! [`ContentFileRepository`] as the shared [`ContentFileLinks`] contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_extension::content_files::{
    ContentFileLinks, ContentFileLinksError, ContentFileRole, FileContentLink, LinkedFile,
};
use systemprompt_identifiers::{ContentId, FileId};

use super::ContentFileRepository;
use crate::error::ContentError;

fn links_error(error: ContentError) -> ContentFileLinksError {
    ContentFileLinksError(Box::new(error))
}

#[async_trait]
impl ContentFileLinks for ContentFileRepository {
    async fn link(
        &self,
        content: &ContentId,
        file: &FileId,
        role: ContentFileRole,
        display_order: i32,
    ) -> Result<(), ContentFileLinksError> {
        Self::link(self, content, file, role, display_order)
            .await
            .map_err(links_error)
    }

    async fn unlink(
        &self,
        content: &ContentId,
        file: &FileId,
    ) -> Result<(), ContentFileLinksError> {
        Self::unlink(self, content, file).await.map_err(links_error)
    }

    async fn files_of(
        &self,
        content: &ContentId,
    ) -> Result<Vec<LinkedFile>, ContentFileLinksError> {
        Self::files_of(self, content).await.map_err(links_error)
    }

    async fn contents_of(
        &self,
        file: &FileId,
    ) -> Result<Vec<FileContentLink>, ContentFileLinksError> {
        Self::contents_of(self, file).await.map_err(links_error)
    }

    async fn featured(
        &self,
        content: &ContentId,
    ) -> Result<Option<LinkedFile>, ContentFileLinksError> {
        Self::featured(self, content).await.map_err(links_error)
    }

    async fn set_featured(
        &self,
        content: &ContentId,
        file: &FileId,
    ) -> Result<(), ContentFileLinksError> {
        Self::set_featured(self, content, file)
            .await
            .map_err(links_error)
    }
}
