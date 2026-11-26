//! [`ContentFileRepository`]: the links between content items and files
//! (`content_files`), which the content member owns because a link cannot
//! exist without the content item it names.
//!
//! It links and unlinks a file, lists the files of a content item and the
//! content items of a file, and keeps the single featured-image role. The
//! stored files a link names are the `files` member's: it reads them through
//! the shared [`DynLinkedFileSource`], keeping the links in their order and
//! leaving out a file that is gone or deleted. It is provided as the shared
//! [`DynContentFileLinks`] the `content files` commands read.
//!
//! [`DynLinkedFileSource`]: systemprompt_extension::content_files::DynLinkedFileSource
//!
//! [`DynContentFileLinks`]: systemprompt_extension::content_files::DynContentFileLinks
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod links;

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use sqlx::types::Uuid;
use systemprompt_database::DbPool;
use systemprompt_extension::content_files::{
    ContentFileRole, DynLinkedFileSource, FileContentLink, LinkedFile, StoredFile,
};
use systemprompt_identifiers::{ContentId, FileId};

use crate::error::{ContentError, ContentResult};

#[derive(Clone)]
pub struct ContentFileRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    files: DynLinkedFileSource,
}

impl std::fmt::Debug for ContentFileRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentFileRepository")
            .finish_non_exhaustive()
    }
}

struct Link {
    file: FileId,
    role: String,
    display_order: i32,
}

fn file_uuid(id: &FileId) -> ContentResult<Uuid> {
    Uuid::parse_str(id.as_str()).map_err(|source| ContentError::InvalidFileId {
        id: id.clone(),
        source,
    })
}

impl ContentFileRepository {
    pub fn new(db: &DbPool, files: DynLinkedFileSource) -> Self {
        Self {
            pool: db.pool(),
            write_pool: db.write_pool(),
            files,
        }
    }

    pub async fn link(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
        role: ContentFileRole,
        display_order: i32,
    ) -> ContentResult<()> {
        let file = file_uuid(file_id)?;
        sqlx::query!(
            r#"
            INSERT INTO content_files (content_id, file_id, role, display_order, created_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
            ON CONFLICT (content_id, file_id, role) DO UPDATE
            SET display_order = $4
            "#,
            content_id as _,
            file,
            role.as_str(),
            display_order,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn unlink(&self, content_id: &ContentId, file_id: &FileId) -> ContentResult<()> {
        let file = file_uuid(file_id)?;
        sqlx::query!(
            "DELETE FROM content_files WHERE content_id = $1 AND file_id = $2",
            content_id as _,
            file,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn files_of(&self, content_id: &ContentId) -> ContentResult<Vec<LinkedFile>> {
        let links = sqlx::query!(
            r#"
            SELECT file_id, role, display_order
            FROM content_files
            WHERE content_id = $1
            ORDER BY display_order ASC, created_at ASC
            "#,
            content_id as _,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        let links: Vec<Link> = links
            .into_iter()
            .map(|row| Link {
                file: FileId::from_uuid(row.file_id),
                role: row.role,
                display_order: row.display_order,
            })
            .collect();
        self.with_files(links).await
    }

    pub async fn featured(&self, content_id: &ContentId) -> ContentResult<Option<LinkedFile>> {
        let links = sqlx::query!(
            r#"
            SELECT file_id, display_order
            FROM content_files
            WHERE content_id = $1 AND role = $2
            ORDER BY display_order ASC, created_at ASC
            "#,
            content_id as _,
            ContentFileRole::Featured.as_str(),
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        let links = links
            .into_iter()
            .map(|row| Link {
                file: FileId::from_uuid(row.file_id),
                role: ContentFileRole::Featured.as_str().to_owned(),
                display_order: row.display_order,
            })
            .collect();
        Ok(self.with_files(links).await?.into_iter().next())
    }

    async fn with_files(&self, links: Vec<Link>) -> ContentResult<Vec<LinkedFile>> {
        if links.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<FileId> = links.iter().map(|link| link.file.clone()).collect();
        let stored: HashMap<FileId, StoredFile> = self
            .files
            .live_files(&ids)
            .await
            .map_err(ContentError::FileSource)?
            .into_iter()
            .map(|file| (file.file_id.clone(), file))
            .collect();
        Ok(links
            .into_iter()
            .filter_map(|link| {
                stored.get(&link.file).map(|file| LinkedFile {
                    file_id: file.file_id.clone(),
                    path: file.path.clone(),
                    public_url: file.public_url.clone(),
                    mime_type: file.mime_type.clone(),
                    size_bytes: file.size_bytes,
                    ai_content: file.ai_content,
                    created_at: file.created_at,
                    role: link.role,
                    display_order: link.display_order,
                })
            })
            .collect())
    }

    pub async fn set_featured(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
    ) -> ContentResult<()> {
        let file = file_uuid(file_id)?;
        let mut tx = self.write_pool.begin().await?;
        sqlx::query!(
            "UPDATE content_files SET role = $1 WHERE content_id = $2 AND role = $3",
            ContentFileRole::Attachment.as_str(),
            content_id as _,
            ContentFileRole::Featured.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query!(
            "UPDATE content_files SET role = $1 WHERE file_id = $2 AND content_id = $3",
            ContentFileRole::Featured.as_str(),
            file,
            content_id as _,
        )
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(ContentError::FileNotLinked {
                file: file_id.clone(),
                content: content_id.clone(),
            });
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn contents_of(&self, file_id: &FileId) -> ContentResult<Vec<FileContentLink>> {
        let file = file_uuid(file_id)?;
        let rows = sqlx::query!(
            r#"
            SELECT content_id, role, display_order, created_at
            FROM content_files
            WHERE file_id = $1
            ORDER BY created_at ASC
            "#,
            file,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| FileContentLink {
                content_id: ContentId::new(row.content_id),
                role: row.role,
                display_order: row.display_order,
                created_at: row.created_at,
            })
            .collect())
    }
}
