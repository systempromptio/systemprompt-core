//! [`NotificationRuleRepository`]: declared rules (no owner, written by the
//! config plane) and each user's own rules. A rule's owner and source never
//! change after it is written: replacing a user's rules refuses an id a
//! declared rule holds and answers that id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use sqlx::PgExecutor;
use sqlx::types::Json;
use systemprompt_identifiers::{NotificationRuleId, UserId};
use systemprompt_models::notifications::{RuleAction, RuleDecl, RuleMatch};
use systemprompt_traits::RepositoryError;

use super::Pools;

struct RuleRow {
    id: NotificationRuleId,
    name: String,
    priority: i32,
    r#match: Json<RuleMatch>,
    action: Json<RuleAction>,
}

impl From<RuleRow> for RuleDecl {
    fn from(row: RuleRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            priority: row.priority,
            r#match: row.r#match.0,
            action: row.action.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationRuleRepository {
    pools: Pools,
}

impl NotificationRuleRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list_global(
        executor: impl PgExecutor<'_>,
    ) -> Result<Vec<RuleDecl>, RepositoryError> {
        Ok(sqlx::query_as!(
            RuleRow,
            r#"SELECT id::text AS "id!: NotificationRuleId", name, priority,
                      match AS "match!: Json<RuleMatch>", action AS "action!: Json<RuleAction>"
               FROM notification_rules WHERE owner_user_id IS NULL
               ORDER BY priority, id"#
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(RuleDecl::from)
        .collect())
    }

    pub async fn list_for_owner(&self, owner: &UserId) -> Result<Vec<RuleDecl>, RepositoryError> {
        let mut by_owner =
            Self::list_for_owners(&*self.pools.read, std::slice::from_ref(owner)).await?;
        Ok(by_owner.remove(owner).unwrap_or_default())
    }

    pub async fn list_for_owners(
        executor: impl PgExecutor<'_>,
        owners: &[UserId],
    ) -> Result<HashMap<UserId, Vec<RuleDecl>>, RepositoryError> {
        if owners.is_empty() {
            return Ok(HashMap::new());
        }
        let owners: Vec<String> = owners.iter().map(|o| o.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT owner_user_id AS "owner!: UserId", id::text AS "id!: NotificationRuleId",
                      name, priority, match AS "match!: Json<RuleMatch>",
                      action AS "action!: Json<RuleAction>"
               FROM notification_rules WHERE owner_user_id = ANY($1)
               ORDER BY owner_user_id, priority, id"#,
            &owners
        )
        .fetch_all(executor)
        .await?;
        let mut out: HashMap<UserId, Vec<RuleDecl>> = HashMap::new();
        for row in rows {
            out.entry(row.owner).or_default().push(
                RuleRow {
                    id: row.id,
                    name: row.name,
                    priority: row.priority,
                    r#match: row.r#match,
                    action: row.action,
                }
                .into(),
            );
        }
        Ok(out)
    }

    pub async fn replace_owner_rules(
        &self,
        owner: &UserId,
        rules: Vec<RuleDecl>,
        source: &str,
    ) -> Result<Option<NotificationRuleId>, RepositoryError> {
        let mut tx = self.pools.write.begin().await?;
        let ids: Vec<String> = rules.iter().map(|r| r.id.as_str().to_owned()).collect();
        let taken = sqlx::query!(
            r#"SELECT id::text AS "id!: NotificationRuleId", owner_user_id IS NULL AS "declared!"
               FROM notification_rules
               WHERE id = ANY($1::text[]::uuid[]) AND owner_user_id IS DISTINCT FROM $2
               ORDER BY id LIMIT 1"#,
            &ids,
            owner.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(taken) = taken {
            if taken.declared {
                return Ok(Some(taken.id));
            }
            return Err(RepositoryError::conflict(
                "notification_rule",
                &taken.id,
                "the id belongs to another user's rule",
            ));
        }
        sqlx::query!(
            "DELETE FROM notification_rules WHERE owner_user_id = $1",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        for rule in rules {
            insert(&mut *tx, &rule, Some(owner), source).await?;
        }
        tx.commit().await?;
        Ok(None)
    }

    pub async fn upsert_declared(
        executor: impl PgExecutor<'_>,
        rule: &RuleDecl,
        source: &str,
    ) -> Result<(), RepositoryError> {
        let matcher = serde_json::to_value(&rule.r#match)?;
        let action = serde_json::to_value(&rule.action)?;
        let result = sqlx::query!(
            "INSERT INTO notification_rules (id, name, priority, owner_user_id, match, action, source)
             VALUES ($1::text::uuid, $2, $3, NULL, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET
                 name = EXCLUDED.name, priority = EXCLUDED.priority, match = EXCLUDED.match,
                 action = EXCLUDED.action, updated_at = now()
             WHERE notification_rules.owner_user_id IS NULL",
            rule.id.as_str(),
            rule.name,
            rule.priority,
            matcher,
            action,
            source,
        )
        .execute(executor)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::conflict(
                "notification_rule",
                &rule.id,
                "the id belongs to a user's own rule",
            ));
        }
        Ok(())
    }

    pub async fn delete_declared(
        executor: impl PgExecutor<'_>,
        id: &NotificationRuleId,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "DELETE FROM notification_rules WHERE id = $1::text::uuid AND owner_user_id IS NULL",
            id.as_str()
        )
        .execute(executor)
        .await?;
        Ok(())
    }
}

async fn insert(
    executor: impl PgExecutor<'_>,
    rule: &RuleDecl,
    owner: Option<&UserId>,
    source: &str,
) -> Result<(), RepositoryError> {
    let matcher = serde_json::to_value(&rule.r#match)?;
    let action = serde_json::to_value(&rule.action)?;
    sqlx::query!(
        "INSERT INTO notification_rules (id, name, priority, owner_user_id, match, action, source)
         VALUES ($1::text::uuid, $2, $3, $4, $5, $6, $7)",
        rule.id.as_str(),
        rule.name,
        rule.priority,
        owner.map(UserId::as_str),
        matcher,
        action,
        source,
    )
    .execute(executor)
    .await?;
    Ok(())
}
