//! Context insert/update/delete mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;

use super::ContextRepository;
use crate::models::context::ContextKind;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_traits::RepositoryError;

const CONTEXT_TOUCH_INTERVAL_SECS: f64 = 60.0;

impl ContextRepository {
    pub async fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
        kind: ContextKind,
    ) -> Result<ContextId, RepositoryError> {
        let context_id = ContextId::generate();
        let now = Utc::now();
        let session_id_str = session_id.map(SessionId::as_str);

        sqlx::query!(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, created_at, \
             updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $6)",
            context_id.as_str(),
            user_id.as_str(),
            session_id_str,
            name,
            kind.as_str(),
            now
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(context_id)
    }

    // Why: the context id derives from caller-supplied metadata, so the
    // touch is scoped to the owning user — another user's call on the same id
    // must be a no-op rather than a write into their row. The gateway calls
    // this on every turn, so an owned row is only rewritten when its
    // `updated_at` is older than `CONTEXT_TOUCH_INTERVAL_SECS` or it lacks a
    // session; otherwise the statement writes nothing and flushes no WAL.
    pub async fn ensure_context(
        &self,
        params: &systemprompt_traits::EnsureContextParams<'_>,
        kind: ContextKind,
    ) -> Result<(), RepositoryError> {
        let context_id = params.context_id;
        let user_id = params.user_id;
        let now = Utc::now();

        let owned = sqlx::query_scalar!(
            r#"WITH inserted AS (
                INSERT INTO user_contexts (context_id, user_id, session_id, name, kind,
                                           created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $6)
                ON CONFLICT (context_id) DO NOTHING
                RETURNING 1
            ), touched AS (
                UPDATE user_contexts
                SET updated_at = $6,
                    session_id = COALESCE(user_contexts.session_id, $3)
                WHERE context_id = $1
                  AND user_id = $2
                  AND NOT EXISTS (SELECT 1 FROM inserted)
                  AND (updated_at < $6 - make_interval(secs => $7)
                       OR (session_id IS NULL AND $3::text IS NOT NULL))
                RETURNING 1
            )
            SELECT (EXISTS (SELECT 1 FROM inserted)
                    OR EXISTS (SELECT 1 FROM touched)
                    OR EXISTS (SELECT 1 FROM user_contexts
                               WHERE context_id = $1 AND user_id = $2)) AS "owned!""#,
            context_id.as_str(),
            user_id.as_str(),
            params.session_id.map(SessionId::as_str),
            params.name,
            kind.as_str(),
            now,
            CONTEXT_TOUCH_INTERVAL_SECS,
        )
        .fetch_one(&*self.write_pool)
        .await?;

        // Why: a row a concurrent first turn inserted after this statement's
        // snapshot is invisible to its ownership probe, so a negative answer
        // is re-read before it becomes a denial.
        if owned || self.is_context_owned_by(context_id, user_id).await? {
            return Ok(());
        }
        Err(RepositoryError::not_found(
            "context",
            format!("{context_id} for user {user_id}"),
        ))
    }

    async fn is_context_owned_by(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<bool, RepositoryError> {
        let owned = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM user_contexts
                              WHERE context_id = $1 AND user_id = $2) AS "owned!""#,
            context_id.as_str(),
            user_id.as_str(),
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(owned)
    }

    // Why: the legacy context is the one row whose owner may be rebound — it is
    // system-owned and adopted by whichever admin the current profile names.
    // The id is fixed here so no caller can use this path to reassign another
    // user's context.
    pub async fn ensure_legacy_context(
        &self,
        system_admin: &UserId,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();
        let legacy = ContextId::legacy_context_row();
        sqlx::query!(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, created_at, \
             updated_at)
             VALUES ($1, $2, NULL, $3, $4, $5, $5)
             ON CONFLICT (context_id) DO UPDATE
             SET user_id = EXCLUDED.user_id,
                 updated_at = EXCLUDED.updated_at",
            legacy.as_str(),
            system_admin.as_str(),
            "Legacy (pre-context)",
            ContextKind::Legacy.as_str(),
            now
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn get_or_create_cli_context(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        name: &str,
    ) -> Result<ContextId, RepositoryError> {
        let now = Utc::now();

        let adopted = sqlx::query_scalar!(
            r#"UPDATE user_contexts SET session_id = $1, updated_at = $2
             WHERE context_id = (
                 SELECT context_id FROM user_contexts
                 WHERE user_id = $3 AND kind = $4 AND name = $5
                 ORDER BY updated_at DESC LIMIT 1
             )
             RETURNING context_id"#,
            session_id.as_str(),
            now,
            user_id.as_str(),
            ContextKind::CliSession.as_str(),
            name
        )
        .fetch_optional(&*self.write_pool)
        .await?;

        match adopted {
            Some(context_id) => {
                ContextId::try_new(context_id).map_err(|e| RepositoryError::decode("context_id", e))
            },
            None => {
                self.create_context(user_id, Some(session_id), name, ContextKind::CliSession)
                    .await
            },
        }
    }

    pub async fn validate_context_ownership(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query_scalar!(
            "SELECT context_id FROM user_contexts WHERE context_id = $1 AND user_id = $2",
            context_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        match result {
            Some(_) => Ok(()),
            None => Err(RepositoryError::not_found(
                "context",
                format!("{context_id} for user {user_id}"),
            )),
        }
    }

    pub async fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();

        let result = sqlx::query!(
            "UPDATE user_contexts SET name = $1, updated_at = $2
             WHERE context_id = $3 AND user_id = $4",
            name,
            now,
            context_id.as_str(),
            user_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::not_found(
                "context",
                format!("{context_id} for user {user_id}"),
            ));
        }

        Ok(())
    }

    pub async fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM user_contexts WHERE context_id = $1 AND user_id = $2",
            context_id.as_str(),
            user_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::not_found(
                "context",
                format!("{context_id} for user {user_id}"),
            ));
        }

        Ok(())
    }
}
