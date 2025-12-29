use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_models::ConversationEvaluation;

#[derive(Debug, Clone)]
pub struct AnalyticsRepository {
    pool: Arc<PgPool>,
}

impl AnalyticsRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_evaluation_metrics(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                primary_category,
                COUNT(*) as "count!: i64",
                AVG(overall_score)::float8 as "avg_score: f64",
                MIN(overall_score)::float8 as "min_score: f64",
                MAX(overall_score)::float8 as "max_score: f64"
            FROM conversation_evaluations
            WHERE DATE(analyzed_at) >= $1 AND DATE(analyzed_at) <= $2
            GROUP BY primary_category
            "#,
            start_date,
            end_date
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "evaluation_type": row.primary_category,
                    "count": row.count,
                    "avg_score": row.avg_score,
                    "min_score": row.min_score,
                    "max_score": row.max_score,
                })
            })
            .collect())
    }

    pub async fn get_low_scoring_conversations(
        &self,
        score_threshold: f64,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let threshold = score_threshold as f32;
        let rows = sqlx::query!(
            r#"
            SELECT id, context_id, overall_score, evaluation_summary, analyzed_at
            FROM conversation_evaluations
            WHERE overall_score < $1
            ORDER BY overall_score ASC
            LIMIT $2
            "#,
            threshold,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.id,
                    "context_id": row.context_id,
                    "overall_score": row.overall_score,
                    "evaluation_summary": row.evaluation_summary,
                    "analyzed_at": row.analyzed_at,
                })
            })
            .collect())
    }

    pub async fn get_top_issues_encountered(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT issues_encountered, COUNT(*) as "count!: i64"
            FROM conversation_evaluations
            WHERE issues_encountered IS NOT NULL
                AND analyzed_at >= $1
                AND analyzed_at <= $2
            GROUP BY issues_encountered
            ORDER BY 2 DESC
            LIMIT 20
            "#,
            start_date,
            end_date
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "issue": row.issues_encountered,
                    "count": row.count,
                })
            })
            .collect())
    }

    pub async fn get_goal_achievement_stats(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT goal_achievement_confidence, COUNT(*) as "count!: i64"
            FROM conversation_evaluations
            WHERE analyzed_at >= $1 AND analyzed_at <= $2
            GROUP BY goal_achievement_confidence
            ORDER BY goal_achievement_confidence
            "#,
            start_date,
            end_date
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "confidence": row.goal_achievement_confidence,
                    "count": row.count,
                })
            })
            .collect())
    }

    pub async fn get_detailed_evaluations(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<ConversationEvaluation>> {
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
            WHERE analyzed_at >= $1 AND analyzed_at <= $2
            ORDER BY analyzed_at DESC
            LIMIT $3 OFFSET $4
            "#,
            start_date,
            end_date,
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_evaluation_quality_distribution(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                CASE
                    WHEN overall_score >= 0.9 THEN 'excellent'
                    WHEN overall_score >= 0.7 THEN 'good'
                    WHEN overall_score >= 0.5 THEN 'fair'
                    ELSE 'poor'
                END as quality_bucket,
                COUNT(*) as "count!: i64"
            FROM conversation_evaluations
            WHERE analyzed_at >= $1 AND analyzed_at <= $2
            GROUP BY quality_bucket
            "#,
            start_date,
            end_date
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "quality_bucket": row.quality_bucket,
                    "count": row.count,
                })
            })
            .collect())
    }
}
