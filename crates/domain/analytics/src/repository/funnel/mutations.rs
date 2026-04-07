use anyhow::Result;
use chrono::Utc;
use systemprompt_identifiers::{FunnelId, FunnelProgressId, SessionId};

use super::FunnelRepository;
use crate::models::{CreateFunnelInput, Funnel, FunnelProgress, FunnelStep, FunnelWithSteps};

impl FunnelRepository {
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
        .execute(&*self.write_pool)
        .await?;

        let mut funnel_ids_arr = Vec::with_capacity(input.steps.len());
        let mut step_orders = Vec::with_capacity(input.steps.len());
        let mut names = Vec::with_capacity(input.steps.len());
        let mut patterns = Vec::with_capacity(input.steps.len());
        let mut match_types = Vec::with_capacity(input.steps.len());
        let mut steps = Vec::with_capacity(input.steps.len());

        for (idx, step_input) in input.steps.iter().enumerate() {
            let step_order = i32::try_from(idx).unwrap_or(0);
            funnel_ids_arr.push(funnel_id.as_str().to_string());
            step_orders.push(step_order);
            names.push(step_input.name.clone());
            patterns.push(step_input.match_pattern.clone());
            match_types.push(step_input.match_type.as_str().to_string());

            steps.push(FunnelStep {
                funnel_id: funnel_id.clone(),
                step_order,
                name: step_input.name.clone(),
                match_pattern: step_input.match_pattern.clone(),
                match_type: step_input.match_type,
            });
        }

        if !steps.is_empty() {
            sqlx::query!(
                r#"
                INSERT INTO funnel_steps (funnel_id, step_order, name, match_pattern, match_type)
                SELECT * FROM UNNEST($1::text[], $2::int4[], $3::text[], $4::text[], $5::text[])
                "#,
                &funnel_ids_arr,
                &step_orders,
                &names,
                &patterns,
                &match_types
            )
            .execute(&*self.write_pool)
            .await?;
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

    pub async fn deactivate(&self, id: &FunnelId) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE funnels SET is_active = FALSE, updated_at = $2 WHERE id = $1
            "#,
            id.as_str(),
            Utc::now()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(&self, id: &FunnelId) -> Result<bool> {
        let result = sqlx::query!(r#"DELETE FROM funnels WHERE id = $1"#, id.as_str())
            .execute(&*self.write_pool)
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
                    .unwrap_or_else(Vec::new);
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
                .execute(&*self.write_pool)
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
        .execute(&*self.write_pool)
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
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
