//! Moving `federated_identities` rows from one issuer namespace to another.
//!
//! A link is keyed `(issuer, external_sub)`. When the issuer a set of links
//! was recorded under no longer resolves them, these operations either move
//! them to the issuer that now does, keeping `user_id`, or remove them so the
//! next sign-in resolves afresh. A row whose `external_sub` the target issuer
//! already links is removed rather than moved, since the target row is the
//! one sign-in now finds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Acquire;

use crate::error::Result;
use crate::repository::UserRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IssuerRekeyPlan {
    pub rows: i64,
    pub already_linked_at_target: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IssuerRekeyOutcome {
    pub moved: u64,
    pub removed: u64,
}

impl UserRepository {
    pub async fn plan_federated_issuer_rekey(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<IssuerRekeyPlan> {
        let row = sqlx::query!(
            r#"SELECT
                COUNT(*) AS "rows!",
                COUNT(*) FILTER (WHERE EXISTS (
                    SELECT 1 FROM federated_identities g
                    WHERE g.issuer = $2 AND g.external_sub = f.external_sub
                )) AS "already_linked_at_target!"
            FROM federated_identities f
            WHERE f.issuer = $1"#,
            from,
            to
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(IssuerRekeyPlan {
            rows: row.rows,
            already_linked_at_target: row.already_linked_at_target,
        })
    }

    pub async fn move_federated_issuer(&self, from: &str, to: &str) -> Result<IssuerRekeyOutcome> {
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;
        let moved = sqlx::query!(
            "UPDATE federated_identities f SET issuer = $2
             WHERE f.issuer = $1 AND NOT EXISTS (
                 SELECT 1 FROM federated_identities g
                 WHERE g.issuer = $2 AND g.external_sub = f.external_sub
             )",
            from,
            to
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let removed = sqlx::query!("DELETE FROM federated_identities WHERE issuer = $1", from)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(IssuerRekeyOutcome { moved, removed })
    }

    pub async fn remove_federated_issuer(&self, from: &str) -> Result<IssuerRekeyOutcome> {
        let removed = sqlx::query!("DELETE FROM federated_identities WHERE issuer = $1", from)
            .execute(&*self.write_pool)
            .await?
            .rows_affected();
        Ok(IssuerRekeyOutcome { moved: 0, removed })
    }
}
