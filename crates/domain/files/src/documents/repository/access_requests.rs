//! [`DocumentAccessRequestRepository`]: requests for access a user with no
//! level sends a document's owner. A user holds at most one open request per
//! document; a second is not stored. Answering is a compare-and-set from
//! `open`, so exactly one answer lands;
//! [`DocumentAccessRequestRepository::decide_in`] runs it inside a caller's
//! transaction so a grant can commit with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentAccessRequestId, DocumentId, UserId};
use systemprompt_models::documents::AccessRequestStatus;
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentAccessRequest, NewAccessRequest};

#[derive(Debug, Clone)]
pub struct DocumentAccessRequestRepository {
    pools: Pools,
}

struct RequestRow {
    id: DocumentAccessRequestId,
    document_id: DocumentId,
    requester_user_id: UserId,
    access_level: String,
    message: Option<String>,
    status: String,
    decided_by: Option<UserId>,
    decided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl TryFrom<RequestRow> for DocumentAccessRequest {
    type Error = RepositoryError;

    fn try_from(row: RequestRow) -> RepoResult<Self> {
        Ok(Self {
            level: parse("document_access_requests.access_level", &row.access_level)?,
            status: parse("document_access_requests.status", &row.status)?,
            id: row.id,
            document_id: row.document_id,
            requester_user_id: row.requester_user_id,
            message: row.message,
            decided_by: row.decided_by,
            decided_at: row.decided_at,
            created_at: row.created_at,
        })
    }
}

impl DocumentAccessRequestRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pools: Pools::new(db),
        }
    }

    pub async fn create(
        &self,
        request: &NewAccessRequest,
    ) -> RepoResult<Option<DocumentAccessRequest>> {
        sqlx::query_as!(
            RequestRow,
            r#"INSERT INTO document_access_requests
                   (document_id, requester_user_id, access_level, message)
               VALUES ($1::text::uuid, $2, $3, $4)
               ON CONFLICT (document_id, requester_user_id) WHERE status = 'open' DO NOTHING
               RETURNING id::text AS "id!: DocumentAccessRequestId",
                         document_id::text AS "document_id!: DocumentId",
                         requester_user_id AS "requester_user_id: UserId", access_level,
                         message, status, decided_by AS "decided_by: UserId", decided_at,
                         created_at"#,
            request.document_id.as_str(),
            request.requester_user_id.as_str(),
            request.level.as_str(),
            request.message,
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentAccessRequest::try_from)
        .transpose()
    }

    pub async fn list_open(&self, document: &DocumentId) -> RepoResult<Vec<DocumentAccessRequest>> {
        sqlx::query_as!(
            RequestRow,
            r#"SELECT id::text AS "id!: DocumentAccessRequestId",
                      document_id::text AS "document_id!: DocumentId",
                      requester_user_id AS "requester_user_id: UserId", access_level, message,
                      status, decided_by AS "decided_by: UserId", decided_at, created_at
               FROM document_access_requests
               WHERE document_id = $1::text::uuid AND status = 'open'
               ORDER BY created_at, id"#,
            document.as_str(),
        )
        .fetch_all(&*self.pools.write)
        .await?
        .into_iter()
        .map(DocumentAccessRequest::try_from)
        .collect()
    }

    pub async fn find(
        &self,
        document: &DocumentId,
        id: &DocumentAccessRequestId,
    ) -> RepoResult<Option<DocumentAccessRequest>> {
        sqlx::query_as!(
            RequestRow,
            r#"SELECT id::text AS "id!: DocumentAccessRequestId",
                      document_id::text AS "document_id!: DocumentId",
                      requester_user_id AS "requester_user_id: UserId", access_level, message,
                      status, decided_by AS "decided_by: UserId", decided_at, created_at
               FROM document_access_requests
               WHERE document_id = $1::text::uuid AND id = $2::text::uuid"#,
            document.as_str(),
            id.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?
        .map(DocumentAccessRequest::try_from)
        .transpose()
    }

    pub async fn begin(&self) -> RepoResult<Transaction<'static, Postgres>> {
        Ok(self.pools.write.begin().await?)
    }

    pub async fn decide(
        &self,
        id: &DocumentAccessRequestId,
        status: AccessRequestStatus,
        decided_by: &UserId,
    ) -> RepoResult<bool> {
        let mut conn = self.pools.write.acquire().await?;
        Self::decide_in(&mut conn, id, status, decided_by).await
    }

    pub async fn decide_in(
        conn: &mut PgConnection,
        id: &DocumentAccessRequestId,
        status: AccessRequestStatus,
        decided_by: &UserId,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_access_requests
             SET status = $2, decided_by = $3, decided_at = now()
             WHERE id = $1::text::uuid AND status = 'open'",
            id.as_str(),
            status.as_str(),
            decided_by.as_str(),
        )
        .execute(&mut *conn)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
