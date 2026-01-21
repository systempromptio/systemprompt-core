use anyhow::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::FunnelId;

use super::FunnelRepository;
use crate::models::{FunnelStats, FunnelStep, FunnelStepStats};

impl FunnelRepository {
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
