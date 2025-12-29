use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_models::ConversationEvaluation;

#[derive(Debug, Clone)]
pub struct EvaluationRepository {
    pool: Arc<PgPool>,
}

impl EvaluationRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn create_evaluation(&self, eval: &ConversationEvaluation) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO conversation_evaluations (
                context_id, agent_goal, goal_achieved, goal_achievement_confidence,
                goal_achievement_notes, primary_category, topics_discussed, keywords,
                user_satisfied, conversation_quality, quality_notes, issues_encountered,
                agent_name, total_turns, conversation_duration_seconds, user_initiated,
                completion_status, overall_score, evaluation_summary, analysis_version
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            "#,
            eval.context_id.as_str(),
            eval.agent_goal,
            eval.goal_achieved,
            eval.goal_achievement_confidence as f32,
            eval.goal_achievement_notes,
            eval.primary_category,
            eval.topics_discussed,
            eval.keywords,
            eval.user_satisfied,
            eval.conversation_quality,
            eval.quality_notes,
            eval.issues_encountered,
            eval.agent_name,
            eval.total_turns,
            eval.conversation_duration_seconds,
            eval.user_initiated,
            eval.completion_status,
            eval.overall_score as f32,
            eval.evaluation_summary,
            eval.analysis_version
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_evaluation_by_context(
        &self,
        context_id: &str,
    ) -> anyhow::Result<Option<ConversationEvaluation>> {
        sqlx::query_as!(
            ConversationEvaluation,
            r#"
            SELECT id, context_id, agent_goal, goal_achieved,
                   goal_achievement_confidence, goal_achievement_notes,
                   primary_category, topics_discussed, keywords,
                   user_satisfied, conversation_quality, quality_notes, issues_encountered,
                   agent_name, total_turns, conversation_duration_seconds, user_initiated,
                   completion_status, overall_score,
                   evaluation_summary, analyzed_at, analysis_version
            FROM conversation_evaluations
            WHERE context_id = $1
            LIMIT 1
            "#,
            context_id
        )
        .fetch_optional(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_unevaluated_conversations(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT c.context_id, c.created_at
            FROM user_contexts c
            LEFT JOIN conversation_evaluations e ON c.context_id = e.context_id
            WHERE e.id IS NULL
              AND EXISTS (
                SELECT 1 FROM task_messages m
                JOIN agent_tasks t ON m.task_id = t.task_id
                WHERE t.context_id = c.context_id
              )
            ORDER BY c.created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "context_id": row.context_id,
                    "created_at": row.created_at,
                })
            })
            .collect())
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> anyhow::Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_contexts c
            WHERE NOT EXISTS (
                SELECT 1 FROM task_messages m
                JOIN agent_tasks t ON m.task_id = t.task_id
                WHERE t.context_id = c.context_id
            )
            AND c.created_at < NOW() - ($1 || ' hours')::INTERVAL
            "#,
            hours_old.to_string()
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
