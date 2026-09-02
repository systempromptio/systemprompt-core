//! Access-control rule persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::RuleId;

use super::{AccessControlRepository, ExportRuleRow, UpsertRuleParams};
use crate::authz::error::AuthzResult;
use crate::authz::types::{Access, AccessRule, EntityKind, RuleType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainFingerprint {
    pub rule_count: i64,
    pub rules_updated_at: Option<DateTime<Utc>>,
    pub entity_count: i64,
    pub entities_updated_at: Option<DateTime<Utc>>,
}

impl AccessControlRepository {
    pub async fn chain_fingerprint(&self) -> AuthzResult<ChainFingerprint> {
        let row = sqlx::query!(
            r#"
            SELECT
                (SELECT COUNT(*) FROM access_control_rules) AS "rule_count!",
                (SELECT MAX(updated_at) FROM access_control_rules) AS "rules_updated_at?",
                (SELECT COUNT(*) FROM access_control_entities) AS "entity_count!",
                (SELECT MAX(updated_at) FROM access_control_entities) AS "entities_updated_at?"
            "#,
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(ChainFingerprint {
            rule_count: row.rule_count,
            rules_updated_at: row.rules_updated_at,
            entity_count: row.entity_count,
            entities_updated_at: row.entities_updated_at,
        })
    }

    pub async fn list_role_rules_for_export(&self) -> AuthzResult<Vec<ExportRuleRow>> {
        let rows = sqlx::query_as!(
            ExportRuleRow,
            r#"
            SELECT entity_type, entity_id, rule_type, rule_value, access, justification
            FROM access_control_rules
            WHERE rule_type = 'role'
            ORDER BY entity_type, entity_id, access, rule_type, rule_value
            "#,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_rules_for_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
    ) -> AuthzResult<Vec<AccessRule>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, rule_type, rule_value, access, justification
            FROM access_control_rules
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY rule_type, rule_value
            "#,
            entity_type.as_str(),
            entity_id,
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(AccessRule {
                id: RuleId::new(row.id),
                rule_type: RuleType::from(row.rule_type.as_str()),
                rule_value: row.rule_value,
                access: Access::from_str(&row.access)?,
                justification: row.justification,
            });
        }
        Ok(out)
    }

    pub async fn list_rules_bulk(
        &self,
        entity_type: EntityKind,
        entity_ids: &[String],
    ) -> AuthzResult<HashMap<String, Vec<AccessRule>>> {
        let mut out: HashMap<String, Vec<AccessRule>> = HashMap::with_capacity(entity_ids.len());
        for id in entity_ids {
            out.entry(id.clone()).or_default();
        }
        if entity_ids.is_empty() {
            return Ok(out);
        }

        let rows = sqlx::query!(
            r#"
            SELECT entity_id, id, rule_type, rule_value, access, justification
            FROM access_control_rules
            WHERE entity_type = $1 AND entity_id = ANY($2)
            ORDER BY entity_id, rule_type, rule_value
            "#,
            entity_type.as_str(),
            entity_ids,
        )
        .fetch_all(&*self.pool)
        .await?;

        for row in rows {
            let rule = AccessRule {
                id: RuleId::new(row.id),
                rule_type: RuleType::from(row.rule_type.as_str()),
                rule_value: row.rule_value,
                access: Access::from_str(&row.access)?,
                justification: row.justification,
            };
            out.entry(row.entity_id).or_default().push(rule);
        }
        Ok(out)
    }

    pub async fn upsert_rule(&self, params: UpsertRuleParams<'_>) -> AuthzResult<AccessRule> {
        let id = RuleId::generate();
        let rule_type_str = params.rule_type.to_string();
        let access_str = params.access.to_string();
        let row = sqlx::query!(
            r#"
            INSERT INTO access_control_rules
                (id, entity_type, entity_id, rule_type, rule_value, access, justification)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (entity_type, entity_id, rule_type, rule_value)
            DO UPDATE SET
                access = EXCLUDED.access,
                justification = COALESCE(EXCLUDED.justification, access_control_rules.justification),
                updated_at = NOW()
            RETURNING id, rule_type, rule_value, access, justification
            "#,
            id.as_str(),
            params.entity_type.as_str(),
            params.entity_id,
            rule_type_str,
            params.rule_value,
            access_str,
            params.justification,
        )
        .fetch_one(&*self.write_pool)
        .await?;

        Ok(AccessRule {
            id: RuleId::new(row.id),
            rule_type: RuleType::from(row.rule_type.as_str()),
            rule_value: row.rule_value,
            access: Access::from_str(&row.access)?,
            justification: row.justification,
        })
    }

    pub async fn set_justification(
        &self,
        rule_id: &RuleId,
        justification: Option<&str>,
    ) -> AuthzResult<bool> {
        let result = sqlx::query!(
            r#"UPDATE access_control_rules SET justification = $2, updated_at = NOW() WHERE id = $1"#,
            rule_id.as_str(),
            justification,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_rule(&self, rule_id: &RuleId) -> AuthzResult<bool> {
        let result = sqlx::query!(
            r#"DELETE FROM access_control_rules WHERE id = $1"#,
            rule_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
