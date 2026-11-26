//! [`FileRepository`] as the shared [`LinkedFileSource`] the content member's
//! links read their stored files through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_extension::content_files::{ContentFileLinksError, LinkedFileSource, StoredFile};
use systemprompt_identifiers::FileId;

use super::FileRepository;
use crate::error::{FilesError, parse_file_uuid};

fn source_error(error: FilesError) -> ContentFileLinksError {
    ContentFileLinksError(Box::new(error))
}

#[async_trait]
impl LinkedFileSource for FileRepository {
    async fn live_files(&self, ids: &[FileId]) -> Result<Vec<StoredFile>, ContentFileLinksError> {
        let uuids = ids
            .iter()
            .map(parse_file_uuid)
            .collect::<Result<Vec<_>, _>>()
            .map_err(source_error)?;
        let rows = sqlx::query!(
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, created_at
            FROM files
            WHERE id = ANY($1) AND deleted_at IS NULL
            "#,
            &uuids,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|error| source_error(FilesError::from(error)))?;
        Ok(rows
            .into_iter()
            .map(|row| StoredFile {
                file_id: FileId::from_uuid(row.id),
                path: row.path,
                public_url: row.public_url,
                mime_type: row.mime_type,
                size_bytes: row.size_bytes,
                ai_content: row.ai_content,
                created_at: row.created_at,
            })
            .collect())
    }
}
