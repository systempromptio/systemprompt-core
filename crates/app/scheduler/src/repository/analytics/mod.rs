use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct AnalyticsRepository {
    write_pool: Arc<PgPool>,
}

impl AnalyticsRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let write_pool = db.write_pool_arc()?;
        Ok(Self { write_pool })
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> anyhow::Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_contexts
            WHERE context_id IN (
                SELECT uc.context_id
                FROM user_contexts uc
                LEFT JOIN task_messages tm ON uc.context_id = tm.context_id
                WHERE tm.id IS NULL
                AND uc.created_at < NOW() - ($1 || ' hours')::interval
            )
            "#,
            hours_old.to_string()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
