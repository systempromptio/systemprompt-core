//! Owner-scoped experiment cursor traversal independent of mutable timestamps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::ExperimentRepository;
use crate::Result;
use crate::experiments::invalid;
use crate::experiments::records::{ExecutionRecord, ExperimentRecord};
use systemprompt_identifiers::UserId;
impl ExperimentRepository {
    pub async fn list_page(
        &self,
        owner: &UserId,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ExperimentRecord>> {
        if !(1..=100).contains(&limit) || after.is_some_and(|id| id.is_empty() || id.len() > 512) {
            return Err(invalid("Experiment cursor limit must be 1–100"));
        }
        Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: sqlx::types::Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.owner_id=$1 AND ($2::text IS NULL OR e.id>$2) ORDER BY e.id LIMIT $3"#,owner.as_str(),after,i64::from(limit)).fetch_all(&self.pool).await?.into_iter().map(|record|record.0).collect())
    }
}

impl ExperimentRepository {
    pub async fn record(
        &self,
        owner: &UserId,
        id: &systemprompt_identifiers::EvalExperimentId,
    ) -> Result<ExperimentRecord> {
        Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: sqlx::types::Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.owner_id=$1 AND e.id=$2"#,owner.as_str(),id.as_str()).fetch_optional(&self.pool).await?.ok_or_else(||crate::experiments::missing("Experiment unavailable"))?.0)
    }
    pub async fn execution_page(
        &self,
        owner: &UserId,
        id: &systemprompt_identifiers::EvalExperimentId,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ExecutionRecord>> {
        if !(1..=100).contains(&limit) || after.is_some_and(|id| id.is_empty() || id.len() > 512) {
            return Err(invalid("Execution cursor limit must be 1–100"));
        }
        self.record(owner, id).await?;
        Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(x) AS "record!: sqlx::types::Json<ExecutionRecord>" FROM eval_executions x WHERE experiment_id=$1 AND ($2::text IS NULL OR id>$2) ORDER BY id LIMIT $3"#,id.as_str(),after,i64::from(limit)).fetch_all(&self.pool).await?.into_iter().map(|record|record.0).collect())
    }
}
