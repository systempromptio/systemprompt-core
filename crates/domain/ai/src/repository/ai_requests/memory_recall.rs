//! The gateway memory recall descriptor on `ai_requests` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiRequestId;
use systemprompt_traits::RepositoryError;

use super::AiRequestRepository;

impl AiRequestRepository {
    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn update_memory_recall(
        &self,
        id: &AiRequestId,
        descriptor: &str,
    ) -> Result<(), RepositoryError> {
        update_memory_recall_with(self.write_pool(), id, descriptor).await
    }
}

pub(crate) async fn update_memory_recall_with<'e, E>(
    executor: E,
    id: &AiRequestId,
    descriptor: &str,
) -> Result<(), RepositoryError>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query!(
        r#"UPDATE ai_requests SET memory_recall = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2"#,
        descriptor,
        id.as_str()
    )
    .execute(executor)
    .await?;
    Ok(())
}
