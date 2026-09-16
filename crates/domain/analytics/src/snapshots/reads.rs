//! Dashboard reads use retained snapshots; workers refresh bounded standard
//! windows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ranges::Range;
use super::{FeedbackSnapshot, FeedbackSnapshotsRepository, SnapshotHealth, invalid};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ManagedResourceId, UserId};

impl FeedbackSnapshotsRepository {
    pub async fn refresh(
        &self,
        owner: &UserId,
        resources: &[ManagedResourceId],
        now: DateTime<Utc>,
    ) -> crate::Result<i64> {
        if resources.len() > 10000 {
            return Err(invalid("Too many snapshot inventory scopes"));
        }
        let mut tx = self.pool.begin().await?;
        let generation = Self::refresh_in(&mut tx, owner, resources, now).await?;
        tx.commit().await?;
        Ok(generation)
    }
    pub(super) async fn refresh_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        resources: &[ManagedResourceId],
        now: DateTime<Utc>,
    ) -> crate::Result<i64> {
        sqlx::query!(
            "INSERT INTO analytics_snapshot_state(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut **tx)
        .await?;
        let state=sqlx::query!("SELECT generation,fact_generation,day FROM analytics_snapshot_state WHERE owner_id=$1 FOR UPDATE",owner.as_str()).fetch_one(&mut **tx).await?;
        let generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("Snapshot generation overflow"))?;
        let existing:std::collections::BTreeSet<String>=sqlx::query_scalar!("SELECT DISTINCT scope FROM analytics_feedback_snapshots WHERE owner_id=$1 ORDER BY scope LIMIT 10002",owner.as_str()).fetch_all(&mut **tx).await?.into_iter().collect();
        let mut scopes: std::collections::BTreeSet<String> = resources
            .iter()
            .map(|id| id.as_str().to_owned())
            .filter(|scope| !existing.contains(scope))
            .collect();
        if state.day != Some(now.date_naive()) {
            scopes.extend(existing);
            scopes.insert(String::new());
        }
        scopes.extend(sqlx::query_scalar!("SELECT scope FROM analytics_snapshot_dirty WHERE owner_id=$1 ORDER BY scope LIMIT 10002",owner.as_str()).fetch_all(&mut **tx).await?);
        if scopes.is_empty() {
            return Ok(state.generation);
        }
        if scopes.len() > 10001 {
            return Err(invalid("Snapshot scope count exceeds limit"));
        }
        let to = now
            .date_naive()
            .succ_opt()
            .ok_or_else(|| invalid("Invalid snapshot date"))?;
        for scope in scopes {
            for window in [1i32, 7, 30, 90, 365] {
                let from = to - chrono::Duration::days(i64::from(window));
                let snapshot = Self::assemble(
                    tx,
                    owner,
                    &Range {
                        scope: &scope,
                        from,
                        to,
                        generation,
                        fact_generation: state.fact_generation,
                        now,
                    },
                )
                .await?;
                let body = serde_json::to_value(snapshot)?;
                sqlx::query!("INSERT INTO analytics_feedback_snapshots(owner_id,scope,window_days,generation,from_day,to_day,body) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,scope,window_days) DO UPDATE SET generation=EXCLUDED.generation,from_day=EXCLUDED.from_day,to_day=EXCLUDED.to_day,body=EXCLUDED.body",owner.as_str(),&scope,window,generation,from,to,body).execute(&mut **tx).await?;
            }
        }
        sqlx::query!(
            "DELETE FROM analytics_snapshot_dirty WHERE owner_id=$1",
            owner.as_str()
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!("UPDATE analytics_snapshot_state SET generation=$2,generated_at=$3,day=$4,retained_from=$5,last_error=NULL WHERE owner_id=$1",owner.as_str(),generation,now,now.date_naive(),to-chrono::Duration::days(365)).execute(&mut **tx).await?;
        sqlx::query!(
            "SELECT pg_notify('feedback_snapshots',$1)",
            format!("{owner}:{generation}")
        )
        .execute(&mut **tx)
        .await?;
        Ok(generation)
    }
    pub async fn snapshot(
        &self,
        owner: &UserId,
        resource: Option<&ManagedResourceId>,
        window: u32,
    ) -> crate::Result<Option<FeedbackSnapshot>> {
        if ![1, 7, 30, 90, 365].contains(&window) {
            return Err(invalid("Unsupported snapshot window"));
        }
        let scope = resource.map_or("", ManagedResourceId::as_str);
        let window = i32::try_from(window).map_err(|_error| invalid("Invalid snapshot window"))?;
        let value=sqlx::query_scalar!("SELECT body FROM analytics_feedback_snapshots WHERE owner_id=$1 AND scope=$2 AND window_days=$3",owner.as_str(),scope,window).fetch_optional(&self.pool).await?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }
    pub async fn snapshots(
        &self,
        owner: &UserId,
        after: Option<&ManagedResourceId>,
        window: u32,
        limit: u32,
    ) -> crate::Result<Vec<FeedbackSnapshot>> {
        if ![1, 7, 30, 90, 365].contains(&window) || !(1..=100).contains(&limit) {
            return Err(invalid("Invalid snapshot page"));
        }
        let after = after.map_or("", ManagedResourceId::as_str);
        let window = i32::try_from(window).map_err(|_error| invalid("Invalid snapshot window"))?;
        let limit = i64::from(limit);
        let rows=sqlx::query_scalar!("SELECT body FROM analytics_feedback_snapshots WHERE owner_id=$1 AND scope>$2 AND window_days=$3 ORDER BY scope LIMIT $4",owner.as_str(),after,window,limit).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|value| serde_json::from_value(value).map_err(Into::into))
            .collect()
    }
    pub async fn health(&self, owner: &UserId) -> crate::Result<SnapshotHealth> {
        let row=sqlx::query!(r#"SELECT s.generation,s.fact_generation,s.generated_at,s.last_error,
   (SELECT COUNT(*) FROM analytics_fact_changes WHERE owner_id=$1 AND state IN('pending','leased')) AS "pending!",
   (SELECT COALESCE(SUM(pending_count),0)::bigint FROM analytics_ingestion_producers) AS "producers!",
   COALESCE((SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1),0) AS "facts!",
   (SELECT COUNT(*) FROM analytics_snapshot_jobs WHERE owner_id=$1 AND state IN('pending','leased')) AS "jobs!"
   FROM analytics_snapshot_state s WHERE s.owner_id=$1"#,owner.as_str()).fetch_optional(&self.pool).await?;
        Ok(row.map_or_else(
            || SnapshotHealth {
                generation: 0,
                fact_generation: 0,
                generated_at: None,
                last_error: Some("Snapshots have not been initialized".to_owned()),
                pending_changes: 0,
                pending_producer_changes: 0,
                facts_generation: 0,
                pending_jobs: 0,
            },
            |row| SnapshotHealth {
                generation: row.generation,
                fact_generation: row.fact_generation,
                generated_at: row.generated_at,
                last_error: row.last_error,
                pending_changes: row.pending,
                pending_producer_changes: row.producers,
                facts_generation: row.facts,
                pending_jobs: row.jobs,
            },
        ))
    }
}
