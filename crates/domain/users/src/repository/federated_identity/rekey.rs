//! Moving `federated_identities` rows from one issuer namespace to another.
//!
//! A link is keyed `(issuer, external_sub)`. When the issuer a set of links
//! was recorded under no longer resolves them, these operations either move
//! them to the issuer the operator names, keeping `user_id`, or remove them so
//! the next sign-in resolves afresh. The source issuer is matched with and
//! without a trailing slash. A row whose `external_sub` the target already
//! links to the same user is removed, since the target row is the one sign-in
//! now finds; a row the target links to a different user is a conflict and is
//! left in place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::Acquire;

use crate::error::Result;
use crate::repository::UserRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IssuerRekeyPlan {
    pub rows: i64,
    pub already_linked_same_user: i64,
    pub conflicting_users: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IssuerRekeyOutcome {
    pub moved: u64,
    pub removed: u64,
    pub conflicts_kept: u64,
}

fn issuer_variants(issuer: &str) -> Vec<String> {
    let bare = issuer.trim_end_matches('/');
    vec![bare.to_owned(), format!("{bare}/")]
}

impl UserRepository {
    pub async fn plan_federated_issuer_rekey(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<IssuerRekeyPlan> {
        let from = issuer_variants(from);
        let row = sqlx::query!(
            r#"SELECT
                COUNT(*) AS "rows!",
                COUNT(*) FILTER (WHERE EXISTS (
                    SELECT 1 FROM federated_identities g
                    WHERE g.issuer = $2 AND g.external_sub = f.external_sub
                      AND g.user_id = f.user_id
                )) AS "same_user!",
                COUNT(*) FILTER (WHERE EXISTS (
                    SELECT 1 FROM federated_identities g
                    WHERE g.issuer = $2 AND g.external_sub = f.external_sub
                      AND g.user_id <> f.user_id
                )) AS "conflicting!"
            FROM federated_identities f
            WHERE f.issuer = ANY($1)"#,
            &from,
            to
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(IssuerRekeyPlan {
            rows: row.rows,
            already_linked_same_user: row.same_user,
            conflicting_users: row.conflicting,
        })
    }

    pub async fn move_federated_issuer(&self, from: &str, to: &str) -> Result<IssuerRekeyOutcome> {
        let from = issuer_variants(from);
        let mut conn = self.write_pool.acquire().await?;
        let mut tx = conn.begin().await?;
        let removed = sqlx::query!(
            "DELETE FROM federated_identities f
             WHERE f.issuer = ANY($1) AND EXISTS (
                 SELECT 1 FROM federated_identities g
                 WHERE g.issuer = $2 AND g.external_sub = f.external_sub
                   AND g.user_id = f.user_id
             )",
            &from,
            to
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let moved = sqlx::query!(
            "UPDATE federated_identities f SET issuer = $2
             WHERE f.issuer = ANY($1) AND NOT EXISTS (
                 SELECT 1 FROM federated_identities g
                 WHERE g.issuer = $2 AND g.external_sub = f.external_sub
             )",
            &from,
            to
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let conflicts_kept = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!" FROM federated_identities WHERE issuer = ANY($1)"#,
            &from
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(IssuerRekeyOutcome {
            moved,
            removed,
            conflicts_kept: conflicts_kept.unsigned_abs(),
        })
    }

    pub async fn remove_federated_issuer(&self, from: &str) -> Result<IssuerRekeyOutcome> {
        let from = issuer_variants(from);
        let removed = sqlx::query!(
            "DELETE FROM federated_identities WHERE issuer = ANY($1)",
            &from
        )
        .execute(&*self.write_pool)
        .await?
        .rows_affected();
        Ok(IssuerRekeyOutcome {
            moved: 0,
            removed,
            conflicts_kept: 0,
        })
    }
}
