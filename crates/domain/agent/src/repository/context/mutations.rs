use chrono::Utc;

use super::ContextRepository;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_traits::RepositoryError;

impl ContextRepository {
    pub async fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> Result<ContextId, RepositoryError> {
        let context_id = ContextId::generate();
        let now = Utc::now();
        let session_id_str = session_id.map(SessionId::as_str);

        sqlx::query!(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name, created_at, \
             updated_at)
             VALUES ($1, $2, $3, $4, $5, $5)",
            context_id.as_str(),
            user_id.as_str(),
            session_id_str,
            name,
            now
        )
        .execute(&*self.write_pool)
        .await
        .map_err(|e| RepositoryError::database(e))?;

        Ok(context_id)
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
        .await
        .map_err(|e| RepositoryError::database(e))?;

        match result {
            Some(_) => Ok(()),
            None => Err(RepositoryError::NotFound(format!(
                "Context {} not found or user {} does not have access",
                context_id, user_id
            ))),
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
        .await
        .map_err(|e| RepositoryError::database(e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Context {} not found for user {}",
                context_id, user_id
            )));
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
        .await
        .map_err(|e| RepositoryError::database(e))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Context {} not found for user {}",
                context_id, user_id
            )));
        }

        Ok(())
    }
}
