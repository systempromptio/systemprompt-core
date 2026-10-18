//! [`BudgetedDb`]: primary-pool transactions carrying a job's [`DbBudget`].
//!
//! The scheduler inserts one into every job context, built from
//! [`systemprompt_traits::Job::db_budget`]. Every transaction begun through
//! it first sets the budget with `set_config(…, true)` — `SET LOCAL`, so the
//! limits end with the transaction and never leak to the pooled session.
//! A job without a budget gets plain transactions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sqlx::{Postgres, Transaction};
use systemprompt_traits::{DbBudget, RepositoryError};

use super::database::DbPool;

#[derive(Debug, Clone)]
pub struct BudgetedDb {
    db: DbPool,
    budget: Option<DbBudget>,
}

// Why: Postgres takes millisecond GUCs as a 32-bit int.
const MAX_GUC_MILLIS: u128 = 2_147_483_647;

fn millis(duration: Duration) -> String {
    duration.as_millis().clamp(1, MAX_GUC_MILLIS).to_string()
}

impl BudgetedDb {
    #[must_use]
    pub const fn new(db: DbPool, budget: Option<DbBudget>) -> Self {
        Self { db, budget }
    }

    #[must_use]
    pub const fn budget(&self) -> Option<DbBudget> {
        self.budget
    }

    #[must_use]
    pub const fn db(&self) -> &DbPool {
        &self.db
    }

    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>, RepositoryError> {
        let pool = self.db.write_pool();
        let mut tx = pool.begin().await?;
        let Some(budget) = self.budget else {
            return Ok(tx);
        };
        sqlx::query_scalar!(
            r#"SELECT set_config('statement_timeout', $1, true) AS "applied!""#,
            millis(budget.statement_timeout)
        )
        .fetch_one(&mut *tx)
        .await?;
        if let Some(lock_timeout) = budget.lock_timeout {
            sqlx::query_scalar!(
                r#"SELECT set_config('lock_timeout', $1, true) AS "applied!""#,
                millis(lock_timeout)
            )
            .fetch_one(&mut *tx)
            .await?;
        }
        if let Some(work_mem_kb) = budget.work_mem_kb {
            sqlx::query_scalar!(
                r#"SELECT set_config('work_mem', $1, true) AS "applied!""#,
                work_mem_kb.max(64).to_string()
            )
            .fetch_one(&mut *tx)
            .await?;
        }
        Ok(tx)
    }
}
