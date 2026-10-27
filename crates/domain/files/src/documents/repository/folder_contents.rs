//! Folder contents regardless of visibility, for folder deletion.

use systemprompt_identifiers::{DocumentFolderId, DocumentId};

use super::DocumentRepository;
use super::rows::RepoResult;

impl DocumentRepository {
    pub async fn ids_in_folder(
        &self,
        folder: &DocumentFolderId,
        limit: i64,
    ) -> RepoResult<Vec<DocumentId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id::text AS "id!: DocumentId" FROM documents
               WHERE folder_id = $1::text::uuid AND deleted_at IS NULL
               ORDER BY id LIMIT $2"#,
            folder.as_str(),
            limit.max(1),
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }
}
