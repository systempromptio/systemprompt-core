//! [`FileRepository`] as the shared [`ContentFileLinks`] contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_extension::content_files::{
    ContentFileLinks, ContentFileLinksError, ContentFileRole, FileContentLink, LinkedFile,
};
use systemprompt_identifiers::{ContentId, FileId};

use crate::error::FilesError;
use crate::models::{File, FileRole};
use crate::repository::file::FileRepository;

impl From<ContentFileRole> for FileRole {
    fn from(role: ContentFileRole) -> Self {
        match role {
            ContentFileRole::Featured => Self::Featured,
            ContentFileRole::Attachment => Self::Attachment,
            ContentFileRole::Inline => Self::Inline,
            ContentFileRole::OgImage => Self::OgImage,
            ContentFileRole::Thumbnail => Self::Thumbnail,
        }
    }
}

fn links_error(error: FilesError) -> ContentFileLinksError {
    ContentFileLinksError(Box::new(error))
}

fn linked(file: File, role: String, display_order: i32) -> LinkedFile {
    LinkedFile {
        file_id: file.id,
        path: file.path,
        public_url: file.public_url,
        mime_type: file.mime_type,
        size_bytes: file.size_bytes,
        ai_content: file.ai_content,
        created_at: file.created_at,
        role,
        display_order,
    }
}

#[async_trait]
impl ContentFileLinks for FileRepository {
    async fn link(
        &self,
        content: &ContentId,
        file: &FileId,
        role: ContentFileRole,
        display_order: i32,
    ) -> Result<(), ContentFileLinksError> {
        self.link_to_content(content, file, role.into(), display_order)
            .await
            .map(drop)
            .map_err(links_error)
    }

    async fn unlink(
        &self,
        content: &ContentId,
        file: &FileId,
    ) -> Result<(), ContentFileLinksError> {
        self.unlink_from_content(content, file)
            .await
            .map_err(links_error)
    }

    async fn files_of(
        &self,
        content: &ContentId,
    ) -> Result<Vec<LinkedFile>, ContentFileLinksError> {
        let rows = self
            .list_files_by_content(content)
            .await
            .map_err(links_error)?;
        Ok(rows
            .into_iter()
            .map(|(file, link)| linked(file, link.role.to_string(), link.display_order))
            .collect())
    }

    async fn contents_of(
        &self,
        file: &FileId,
    ) -> Result<Vec<FileContentLink>, ContentFileLinksError> {
        let rows = self.list_content_by_file(file).await.map_err(links_error)?;
        Ok(rows
            .into_iter()
            .map(|link| FileContentLink {
                content_id: link.content_id,
                role: link.role.to_string(),
                display_order: link.display_order,
                created_at: link.created_at,
            })
            .collect())
    }

    async fn featured(
        &self,
        content: &ContentId,
    ) -> Result<Option<LinkedFile>, ContentFileLinksError> {
        let file = self
            .find_featured_image(content)
            .await
            .map_err(links_error)?;
        Ok(file.map(|file| linked(file, FileRole::Featured.to_string(), 0)))
    }

    async fn set_featured(
        &self,
        content: &ContentId,
        file: &FileId,
    ) -> Result<(), ContentFileLinksError> {
        Self::set_featured(self, file, content)
            .await
            .map_err(links_error)
    }
}
