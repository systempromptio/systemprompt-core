//! Transactional reservations retain uncertain spend and settle each request
//! once.
//!
//! Orphan retention reads the recorded usage of each unsettled request through
//! `AiRequestTrace`; an execution paused for approval is live and its
//! reservations stay held.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::experiments::records::BudgetRecord;
use crate::{EvaluationError, Result};
use sqlx::PgPool;
use std::collections::BTreeMap;
use systemprompt_identifiers::{AiRequestId, EvalBudgetId, EvalReservationId, UserId};
use systemprompt_traits::{DynAiRequestTrace, TraceRequestUsage};

use crate::experiments::invalid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationAdmission {
    Admitted(EvalReservationId),
    AlreadyReserved(EvalReservationId),
}

#[derive(Clone)]
pub struct BudgetRepository {
    pool: PgPool,
    trace: DynAiRequestTrace,
}

impl std::fmt::Debug for BudgetRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BudgetRepository").finish_non_exhaustive()
    }
}

impl BudgetRepository {
    pub const fn new(pool: PgPool, trace: DynAiRequestTrace) -> Self {
        Self { pool, trace }
    }

    pub async fn create_shared(
        &self,
        owner: &UserId,
        operation: &str,
        cap: i64,
    ) -> Result<EvalBudgetId> {
        if cap <= 0 || operation.trim().is_empty() || operation.len() > 255 {
            return Err(invalid(
                "Budget requires a positive cap and an idempotency key",
            ));
        }
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let id = EvalBudgetId::generate();
        sqlx::query!("INSERT INTO eval_budget_accounts(id,owner_id,cap,operation_key) VALUES($1,$2,$3,$4) ON CONFLICT(owner_id,operation_key) DO NOTHING",
        id.as_str(), owner.as_str(), cap, operation)
        .execute(&mut *tx)
        .await?;
        let stored = sqlx::query!(
            "SELECT id,cap FROM eval_budget_accounts WHERE owner_id=$1 AND operation_key=$2",
            owner.as_str(),
            operation
        )
        .fetch_one(&mut *tx)
        .await?;
        if stored.cap != cap {
            return Err(crate::experiments::conflict(
                "Budget idempotency key conflicts with another cap",
            ));
        }
        tx.commit().await?;
        Ok(EvalBudgetId::new(stored.id))
    }

    pub async fn get(&self, owner: &UserId, id: &EvalBudgetId) -> Result<BudgetRecord> {
        let record = sqlx::query!(
            "SELECT id,cap,reserved,settled,frozen FROM eval_budget_accounts WHERE owner_id=$1 AND id=$2",
            owner.as_str(), id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| crate::experiments::missing("Budget unavailable in this scope"))?;
        Ok(BudgetRecord {
            id: EvalBudgetId::new(record.id),
            cap: record.cap,
            reserved: record.reserved,
            settled: record.settled,
            frozen: record.frozen,
        })
    }

    pub async fn reserve(
        &self,
        owner: &UserId,
        account: &EvalBudgetId,
        operation: &str,
        amount: i64,
    ) -> Result<ReservationAdmission> {
        let mut tx = self.pool.begin().await?;
        let admitted = Self::reserve_in(&mut tx, owner, account, operation, amount).await?;
        tx.commit().await?;
        Ok(admitted)
    }

    pub(super) async fn reserve_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        account: &EvalBudgetId,
        operation: &str,
        amount: i64,
    ) -> Result<ReservationAdmission> {
        if amount <= 0 || operation.trim().is_empty() || operation.len() > 255 {
            return Err(invalid(
                "A reservation needs a positive bound and a unique operation key",
            ));
        }
        let row = sqlx::query!("SELECT cap,reserved,settled,frozen FROM eval_budget_accounts WHERE id=$1 AND owner_id=$2 FOR UPDATE", account.as_str(), owner.as_str())
            .fetch_optional(&mut **tx).await?
            .ok_or_else(|| crate::experiments::missing("Budget unavailable in this scope"))?;
        let existing = sqlx::query!("SELECT id,reserved FROM eval_budget_reservations WHERE account_id=$1 AND operation_key=$2", account.as_str(), operation)
            .fetch_optional(&mut **tx).await?;
        if let Some(existing) = existing {
            if existing.reserved != amount {
                return Err(crate::experiments::conflict(
                    "Reservation key conflicts with another bound",
                ));
            }
            return Ok(ReservationAdmission::AlreadyReserved(
                EvalReservationId::new(existing.id),
            ));
        }
        let cap: i64 = row.cap;
        let reserved: i64 = row.reserved;
        let settled: i64 = row.settled;
        let total = i128::from(reserved) + i128::from(settled) + i128::from(amount);
        if row.frozen || total > i128::from(cap) {
            return Err(EvaluationError::BudgetExhausted {
                required: amount,
                available: cap.saturating_sub(reserved).saturating_sub(settled),
            });
        }
        let id = EvalReservationId::generate();
        sqlx::query!("INSERT INTO eval_budget_reservations(id,account_id,operation_key,reserved) VALUES($1,$2,$3,$4)", id.as_str(), account.as_str(), operation, amount)
            .execute(&mut **tx).await?;
        sqlx::query!(
            "UPDATE eval_budget_accounts SET reserved=reserved+$2 WHERE id=$1",
            account.as_str(),
            amount
        )
        .execute(&mut **tx)
        .await?;
        Ok(ReservationAdmission::Admitted(id))
    }

    pub async fn retain_orphaned(&self, owner: &UserId) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let orphaned = sqlx::query!(
            r#"SELECT r.id,r.account_id,r.reserved,m.request_id
                FROM eval_budget_reservations r
                JOIN eval_budget_accounts a ON a.id=r.account_id AND a.owner_id=$1
                JOIN eval_request_reservations m ON m.reservation_id=r.id
                JOIN eval_executions x ON x.id=m.execution_id
                WHERE r.actual IS NULL AND x.status NOT IN ('queued','running','awaiting_approval')
                FOR UPDATE OF r,a"#,
            owner.as_str()
        )
        .fetch_all(&mut *tx)
        .await?;
        let request_ids: Vec<AiRequestId> = orphaned
            .iter()
            .map(|row| AiRequestId::new(row.request_id.clone()))
            .collect();
        let recorded: BTreeMap<String, i64> = self
            .trace
            .list_usage(owner, &request_ids)
            .await?
            .into_iter()
            .filter(TraceRequestUsage::is_settled)
            .map(|usage| {
                (
                    usage.request_id.as_str().to_owned(),
                    usage.cost_microdollars,
                )
            })
            .collect();
        let mut retained = 0u64;
        for row in orphaned {
            let actual = recorded
                .get(&row.request_id)
                .copied()
                .unwrap_or(row.reserved);
            sqlx::query!(
                "UPDATE eval_budget_reservations SET actual=$2,request_id=$3,settled_at=NOW() WHERE id=$1",
                row.id,
                actual,
                row.request_id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE eval_budget_accounts SET reserved=reserved-$2,settled=settled+$3,frozen=frozen OR $3>$2 WHERE id=$1",
                row.account_id,
                row.reserved,
                actual
            )
            .execute(&mut *tx)
            .await?;
            retained += 1;
        }
        tx.commit().await?;
        Ok(retained)
    }

    pub async fn settle(
        &self,
        owner: &UserId,
        reservation: &EvalReservationId,
        request: &AiRequestId,
        actual: i64,
    ) -> Result<()> {
        if actual < 0 {
            return Err(invalid("Actual spend cannot be negative"));
        }
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!("SELECT r.account_id,r.reserved,r.actual,r.request_id FROM eval_budget_reservations r JOIN eval_budget_accounts a ON a.id=r.account_id WHERE r.id=$1 AND a.owner_id=$2 FOR UPDATE OF a,r", reservation.as_str(), owner.as_str())
            .fetch_optional(&mut *tx).await?
            .ok_or_else(|| crate::experiments::missing("Reservation unavailable in this scope"))?;
        if let Some(previous) = row.actual {
            if previous != actual || row.request_id.as_deref() != Some(request.as_str()) {
                return Err(invalid(
                    "Settlement conflicts with previously recorded spend",
                ));
            }
            return Ok(());
        }
        let reserved: i64 = row.reserved;
        sqlx::query!("UPDATE eval_budget_reservations SET actual=$2,request_id=$3,settled_at=NOW() WHERE id=$1", reservation.as_str(), actual, request.as_str())
            .execute(&mut *tx).await?;
        sqlx::query!("UPDATE eval_budget_accounts SET reserved=reserved-$2,settled=settled+$3,frozen=frozen OR $3>$2 WHERE id=$1", row.account_id, reserved, actual)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
