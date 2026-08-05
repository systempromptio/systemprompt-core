//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::EvalRubricId;

use crate::error::{EvaluationError, Result};
use crate::models::{Rubric, RubricDimension};

#[derive(Debug, Clone)]
pub struct EvalRubricRepository {
    pool: Arc<PgPool>,
}

impl EvalRubricRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?,
        })
    }

    pub async fn upsert(&self, rubric: &Rubric) -> Result<()> {
        let dimensions = serde_json::to_value(&rubric.dimensions)?;
        sqlx::query!(
            r#"
            INSERT INTO eval_rubrics (id, name, dimensions, pass_threshold, prompt_template, enabled)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (name) DO UPDATE
            SET dimensions = EXCLUDED.dimensions,
                pass_threshold = EXCLUDED.pass_threshold,
                prompt_template = EXCLUDED.prompt_template,
                enabled = EXCLUDED.enabled
            "#,
            rubric.id.as_str(),
            rubric.name,
            dimensions,
            rubric.pass_threshold,
            rubric.prompt_template.as_deref(),
            rubric.enabled
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Rubric> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, dimensions, pass_threshold, prompt_template, enabled
            FROM eval_rubrics
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or_else(|| EvaluationError::RubricNotFound(name.to_owned()))?;

        let dimensions: Vec<RubricDimension> = serde_json::from_value(row.dimensions)?;
        Ok(Rubric {
            id: EvalRubricId::new(row.id),
            name: row.name,
            dimensions,
            pass_threshold: row.pass_threshold,
            prompt_template: row.prompt_template,
            enabled: row.enabled,
        })
    }
}
