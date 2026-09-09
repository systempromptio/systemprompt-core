//! Transactional reservations retain uncertain spend and settle each request
//! once.

use crate::{EvaluationError, Result};
use sqlx::PgPool;
use systemprompt_identifiers::{AiRequestId, EvalBudgetId, EvalReservationId, UserId};

use crate::experiments::invalid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationAdmission {
    Admitted(EvalReservationId),
    AlreadyReserved(EvalReservationId),
}

#[derive(Debug, Clone)]
pub struct BudgetRepository {
    pool: PgPool,
}

impl BudgetRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, owner: &UserId, cap: i64) -> Result<EvalBudgetId> {
        if cap <= 0 {
            return Err(invalid("Budget must be positive"));
        }
        let id = EvalBudgetId::generate();
        sqlx::query!(
            "INSERT INTO eval_budget_accounts(id,owner_id,cap) VALUES($1,$2,$3)",
            id.as_str(),
            owner.as_str(),
            cap
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn reserve(
        &self,
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
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!("SELECT cap,reserved,settled,frozen FROM eval_budget_accounts WHERE id=$1 AND owner_id=$2 FOR UPDATE", account.as_str(), owner.as_str())
            .fetch_optional(&mut *tx).await?
            .ok_or_else(|| crate::experiments::missing("Budget unavailable in this scope"))?;
        let existing = sqlx::query!("SELECT id,reserved FROM eval_budget_reservations WHERE account_id=$1 AND operation_key=$2", account.as_str(), operation)
            .fetch_optional(&mut *tx).await?;
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
                spent: settled,
                budget: cap,
            });
        }
        let id = EvalReservationId::generate();
        sqlx::query!("INSERT INTO eval_budget_reservations(id,account_id,operation_key,reserved) VALUES($1,$2,$3,$4)", id.as_str(), account.as_str(), operation, amount)
            .execute(&mut *tx).await?;
        sqlx::query!(
            "UPDATE eval_budget_accounts SET reserved=reserved+$2 WHERE id=$1",
            account.as_str(),
            amount
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ReservationAdmission::Admitted(id))
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
