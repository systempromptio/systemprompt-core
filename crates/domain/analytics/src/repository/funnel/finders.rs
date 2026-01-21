use anyhow::Result;
use systemprompt_identifiers::{FunnelId, SessionId};

use super::types::{FunnelProgressRow, FunnelRow, FunnelStepRow};
use super::FunnelRepository;
use crate::models::{Funnel, FunnelProgress, FunnelStep, FunnelWithSteps};

impl FunnelRepository {
    pub async fn find_by_id(&self, id: &FunnelId) -> Result<Option<FunnelWithSteps>> {
        let funnel_row = sqlx::query_as!(
            FunnelRow,
            r#"
            SELECT id, name, description, is_active, created_at, updated_at
            FROM funnels WHERE id = $1
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
            FROM funnels WHERE name = $1
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
            FROM funnels WHERE is_active = TRUE ORDER BY name
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
            FROM funnels ORDER BY name
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(FunnelRow::into_funnel).collect())
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
            FROM funnel_progress WHERE funnel_id = $1 AND session_id = $2
            "#,
            funnel_id.as_str(),
            session_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(FunnelProgressRow::into_progress))
    }

    pub(super) async fn get_steps_for_funnel(
        &self,
        funnel_id: &FunnelId,
    ) -> Result<Vec<FunnelStep>> {
        let rows = sqlx::query_as!(
            FunnelStepRow,
            r#"
            SELECT funnel_id, step_order, name, match_pattern, match_type
            FROM funnel_steps WHERE funnel_id = $1 ORDER BY step_order
            "#,
            funnel_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(FunnelStepRow::into_step).collect())
    }
}
