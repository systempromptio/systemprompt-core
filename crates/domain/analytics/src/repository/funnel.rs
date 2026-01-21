use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{FunnelId, FunnelProgressId, SessionId};

use crate::models::{
    CreateFunnelInput, Funnel, FunnelMatchType, FunnelProgress, FunnelStats, FunnelStep,
    FunnelStepStats, FunnelWithSteps,
};

#[derive(Clone, Debug)]
pub struct FunnelRepository {
    pool: Arc<PgPool>,
}

impl FunnelRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn create_funnel(&self, input: &CreateFunnelInput) -> Result<FunnelWithSteps> {
        let funnel_id = FunnelId::generate();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO funnels (id, name, description, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, TRUE, $4, $4)
            "#,
            funnel_id.as_str(),
            input.name,
            input.description,
            now
        )
        .execute(&*self.pool)
        .await?;

        let mut steps = Vec::with_capacity(input.steps.len());
        for (idx, step_input) in input.steps.iter().enumerate() {
            let step_order = i32::try_from(idx).unwrap_or(0);
            let match_type = step_input.match_type.as_str();

            sqlx::query!(
                r#"
                INSERT INTO funnel_steps (funnel_id, step_order, name, match_pattern, match_type)
                VALUES ($1, $2, $3, $4, $5)
                "#,
                funnel_id.as_str(),
                step_order,
                step_input.name,
                step_input.match_pattern,
                match_type
            )
            .execute(&*self.pool)
            .await?;

            steps.push(FunnelStep {
                funnel_id: funnel_id.clone(),
                step_order,
                name: step_input.name.clone(),
                match_pattern: step_input.match_pattern.clone(),
                match_type: step_input.match_type,
            });
        }

        let funnel = Funnel {
            id: funnel_id,
            name: input.name.clone(),
            description: input.description.clone(),
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        Ok(FunnelWithSteps { funnel, steps })
    }

    pub async fn find_by_id(&self, id: &FunnelId) -> Result<Option<FunnelWithSteps>> {
        let funnel_row = sqlx::query_as!(
            FunnelRow,
            r#"
            SELECT id, name, description, is_active, created_at, updated_at
            FROM funnels
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        let Some(row) = funnel_row else {
            return Ok(None);
        };

        let funnel = row.into_funnel();
        let steps = self.get_steps_for_funnel(id).await?;

        Ok(Some(FunnelWithSteps { funnel, steps }))
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<FunnelWithSteps>> {
        let funnel_row = sqlx::query_as!(
            FunnelRow,
            r#"
            SELECT id, name, description, is_active, created_at, updated_at
            FROM funnels
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&*self.pool)
        .await?;

        let Some(row) = funnel_row else {
            return Ok(None);
        };

        let funnel = row.into_funnel();
        let funnel_id = FunnelId::new(funnel.id.as_str());
        let steps = self.get_steps_for_funnel(&funnel_id).await?;

        Ok(Some(FunnelWithSteps { funnel, steps }))
    }

    pub async fn list_active(&self) -> Result<Vec<Funnel>> {
        let rows = sqlx::query_as!(
            FunnelRow,
            r#"
            SELECT id, name, description, is_active, created_at, updated_at
            FROM funnels
            WHERE is_active = TRUE
            ORDER BY name
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(FunnelRow::into_funnel).collect())
    }

    pub async fn list_all(&self) -> Result<Vec<Funnel>> {
        let rows = sqlx::query_as!(
            FunnelRow,
            r#"
            SELECT id, name, description, is_active, created_at, updated_at
            FROM funnels
            ORDER BY name
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(FunnelRow::into_funnel).collect())
    }

    pub async fn deactivate(&self, id: &FunnelId) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE funnels
            SET is_active = FALSE, updated_at = $2
            WHERE id = $1
            "#,
            id.as_str(),
            Utc::now()
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(&self, id: &FunnelId) -> Result<bool> {
        let result = sqlx::query!(r#"DELETE FROM funnels WHERE id = $1"#, id.as_str())
            .execute(&*self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn record_progress(
        &self,
        funnel_id: &FunnelId,
        session_id: &SessionId,
        step: i32,
    ) -> Result<FunnelProgress> {
        let now = Utc::now();
        let step_timestamp = serde_json::json!({
            "step": step,
            "timestamp": now.to_rfc3339()
        });

        if let Some(mut progress) = self.find_progress(funnel_id, session_id).await? {
            if step > progress.current_step {
                let mut timestamps = progress
                    .step_timestamps
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                timestamps.push(step_timestamp);

                sqlx::query!(
                    r#"
                    UPDATE funnel_progress
                    SET current_step = $3, step_timestamps = $4, updated_at = $5
                    WHERE funnel_id = $1 AND session_id = $2
                    "#,
                    funnel_id.as_str(),
                    session_id.as_str(),
                    step,
                    serde_json::Value::Array(timestamps.clone()),
                    now
                )
                .execute(&*self.pool)
                .await?;

                progress.current_step = step;
                progress.step_timestamps = serde_json::Value::Array(timestamps);
                progress.updated_at = now;
            }
            return Ok(progress);
        }

        let id = FunnelProgressId::generate();
        let timestamps = serde_json::json!([step_timestamp]);

        sqlx::query!(
            r#"
            INSERT INTO funnel_progress (
                id, funnel_id, session_id, current_step, step_timestamps, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            "#,
            id.as_str(),
            funnel_id.as_str(),
            session_id.as_str(),
            step,
            timestamps,
            now
        )
        .execute(&*self.pool)
        .await?;

        Ok(FunnelProgress {
            id,
            funnel_id: funnel_id.clone(),
            session_id: session_id.clone(),
            current_step: step,
            completed_at: None,
            dropped_at_step: None,
            step_timestamps: timestamps,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn mark_completed(
        &self,
        funnel_id: &FunnelId,
        session_id: &SessionId,
    ) -> Result<bool> {
        let now = Utc::now();
        let result = sqlx::query!(
            r#"
            UPDATE funnel_progress
            SET completed_at = $3, updated_at = $3
            WHERE funnel_id = $1 AND session_id = $2
            "#,
            funnel_id.as_str(),
            session_id.as_str(),
            now
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn find_progress(
        &self,
        funnel_id: &FunnelId,
        session_id: &SessionId,
    ) -> Result<Option<FunnelProgress>> {
        let row = sqlx::query_as!(
            FunnelProgressRow,
            r#"
            SELECT id, funnel_id, session_id, current_step, completed_at, dropped_at_step,
                   step_timestamps, created_at, updated_at
            FROM funnel_progress
            WHERE funnel_id = $1 AND session_id = $2
            "#,
            funnel_id.as_str(),
            session_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(FunnelProgressRow::into_progress))
    }

    pub async fn get_stats(
        &self,
        funnel_id: &FunnelId,
        since: DateTime<Utc>,
    ) -> Result<FunnelStats> {
        let funnel = self
            .find_by_id(funnel_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Funnel not found: {}", funnel_id))?;

        let total_entries = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM funnel_progress
            WHERE funnel_id = $1 AND created_at >= $2
            "#,
            funnel_id.as_str(),
            since
        )
        .fetch_one(&*self.pool)
        .await?;

        let total_completions = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM funnel_progress
            WHERE funnel_id = $1 AND created_at >= $2 AND completed_at IS NOT NULL
            "#,
            funnel_id.as_str(),
            since
        )
        .fetch_one(&*self.pool)
        .await?;

        let overall_conversion_rate = if total_entries > 0 {
            (total_completions as f64 / total_entries as f64) * 100.0
        } else {
            0.0
        };

        let step_stats = self
            .calculate_step_stats(funnel_id, &funnel.steps, since)
            .await?;

        Ok(FunnelStats {
            funnel_id: funnel_id.clone(),
            funnel_name: funnel.funnel.name,
            total_entries,
            total_completions,
            overall_conversion_rate,
            step_stats,
        })
    }

    async fn get_steps_for_funnel(&self, funnel_id: &FunnelId) -> Result<Vec<FunnelStep>> {
        let rows = sqlx::query_as!(
            FunnelStepRow,
            r#"
            SELECT funnel_id, step_order, name, match_pattern, match_type
            FROM funnel_steps
            WHERE funnel_id = $1
            ORDER BY step_order
            "#,
            funnel_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(FunnelStepRow::into_step).collect())
    }

    async fn calculate_step_stats(
        &self,
        funnel_id: &FunnelId,
        steps: &[FunnelStep],
        since: DateTime<Utc>,
    ) -> Result<Vec<FunnelStepStats>> {
        let mut stats = Vec::with_capacity(steps.len());

        for step in steps {
            let entered_count = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM funnel_progress
                WHERE funnel_id = $1 AND created_at >= $2 AND current_step >= $3
                "#,
                funnel_id.as_str(),
                since,
                step.step_order
            )
            .fetch_one(&*self.pool)
            .await?;

            let exited_count = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM funnel_progress
                WHERE funnel_id = $1 AND created_at >= $2 AND current_step > $3
                "#,
                funnel_id.as_str(),
                since,
                step.step_order
            )
            .fetch_one(&*self.pool)
            .await?;

            let conversion_rate = if entered_count > 0 {
                (exited_count as f64 / entered_count as f64) * 100.0
            } else {
                0.0
            };

            stats.push(FunnelStepStats {
                step_order: step.step_order,
                entered_count,
                exited_count,
                conversion_rate,
                avg_time_to_next_ms: None,
            });
        }

        Ok(stats)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FunnelRow {
    id: String,
    name: String,
    description: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl FunnelRow {
    fn into_funnel(self) -> Funnel {
        Funnel {
            id: FunnelId::new(self.id),
            name: self.name,
            description: self.description,
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FunnelStepRow {
    funnel_id: String,
    step_order: i32,
    name: String,
    match_pattern: String,
    match_type: String,
}

impl FunnelStepRow {
    fn into_step(self) -> FunnelStep {
        FunnelStep {
            funnel_id: FunnelId::new(self.funnel_id),
            step_order: self.step_order,
            name: self.name,
            match_pattern: self.match_pattern,
            match_type: FunnelMatchType::from_str(&self.match_type),
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct FunnelProgressRow {
    id: String,
    funnel_id: String,
    session_id: String,
    current_step: i32,
    completed_at: Option<DateTime<Utc>>,
    dropped_at_step: Option<i32>,
    step_timestamps: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl FunnelProgressRow {
    fn into_progress(self) -> FunnelProgress {
        FunnelProgress {
            id: FunnelProgressId::new(self.id),
            funnel_id: FunnelId::new(self.funnel_id),
            session_id: SessionId::new(self.session_id),
            current_step: self.current_step,
            completed_at: self.completed_at,
            dropped_at_step: self.dropped_at_step,
            step_timestamps: self.step_timestamps,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl FunnelMatchType {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::UrlExact => "url_exact",
            Self::UrlPrefix => "url_prefix",
            Self::UrlRegex => "url_regex",
            Self::EventType => "event_type",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "url_exact" => Self::UrlExact,
            "url_regex" => Self::UrlRegex,
            "event_type" => Self::EventType,
            _ => Self::UrlPrefix,
        }
    }
}
