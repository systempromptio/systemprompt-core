//! Replayable backfill pages enqueue changes and advance checkpoints in one
//! transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{BackfillProgress, FeedbackFactsRepository, validation};
use crate::Result;
use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_models::feedback::ContentDigest;

impl FeedbackFactsRepository {
    pub async fn begin_backfill(
        &self,
        owner: &UserId,
        job: &TaskId,
        source: &str,
    ) -> Result<BackfillProgress> {
        if source.is_empty() || source.len() > 128 || source.chars().any(char::is_control) {
            return Err(validation::invalid());
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!("INSERT INTO analytics_fact_backfills(owner_id,job_id,source) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", owner.as_str(), job.as_str(), source).execute(&mut *tx).await?;
        tx.commit().await?;
        let progress = self.backfill(owner, job).await?;
        if progress.source != source {
            return Err(validation::invalid());
        }
        Ok(progress)
    }

    pub async fn backfill(&self, owner: &UserId, job: &TaskId) -> Result<BackfillProgress> {
        let row = sqlx::query!("SELECT source,cursor,generation,pages,facts,complete FROM analytics_fact_backfills WHERE owner_id=$1 AND job_id=$2", owner.as_str(), job.as_str()).fetch_optional(&self.pool).await?.ok_or_else(validation::invalid)?;
        Ok(BackfillProgress {
            job_id: job.clone(),
            source: row.source,
            cursor: row.cursor,
            generation: row.generation,
            pages: row.pages,
            facts: row.facts,
            complete: row.complete,
        })
    }

    pub async fn append_backfill_page(
        &self,
        owner: &UserId,
        job: &TaskId,
        page: &super::BackfillPage,
    ) -> Result<BackfillProgress> {
        if page.changes.len() > 256
            || page.next_cursor.len() > 1024
            || page.expected_generation < 0
            || (page.changes.is_empty() && !page.complete)
        {
            return Err(validation::invalid());
        }
        let digest = ContentDigest::of(&serde_json::to_vec(page)?);
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;

        let progress = sqlx::query!("SELECT source,generation,complete FROM analytics_fact_backfills WHERE owner_id=$1 AND job_id=$2 FOR UPDATE", owner.as_str(), job.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(validation::invalid)?;
        let previous = sqlx::query_scalar!("SELECT digest FROM analytics_fact_backfill_pages WHERE owner_id=$1 AND job_id=$2 AND page_generation=$3", owner.as_str(), job.as_str(), page.expected_generation).fetch_optional(&mut *tx).await?;
        if let Some(previous) = previous {
            if previous != digest.as_str() {
                return Err(validation::invalid());
            }
            tx.commit().await?;
            return self.backfill(owner, job).await;
        }
        if progress.complete
            || progress.generation != page.expected_generation
            || page
                .changes
                .iter()
                .any(|change| change.key.source != progress.source)
        {
            return Err(validation::invalid());
        }
        for change in &page.changes {
            Self::submit_in(&mut tx, owner, change).await?;
        }
        let count = i64::try_from(page.changes.len()).map_err(|_error| validation::invalid())?;
        sqlx::query!("INSERT INTO analytics_fact_backfill_pages(owner_id,job_id,page_generation,digest) VALUES($1,$2,$3,$4)", owner.as_str(), job.as_str(), page.expected_generation, digest.as_str()).execute(&mut *tx).await?;
        sqlx::query!("UPDATE analytics_fact_backfills SET cursor=$3,generation=generation+1,pages=pages+1,facts=facts+$4,complete=$5,updated_at=clock_timestamp() WHERE owner_id=$1 AND job_id=$2", owner.as_str(), job.as_str(), &page.next_cursor, count, page.complete).execute(&mut *tx).await?;
        tx.commit().await?;
        self.backfill(owner, job).await
    }
}
