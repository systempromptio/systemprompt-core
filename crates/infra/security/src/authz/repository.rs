//! `AccessControlRepository` — sqlx-backed access to `access_control_rules`.
//!
//! Generic over `entity_type` so the same repository serves the gateway
//! (`gateway_route`), MCP (`mcp_server`), and any future enforcement site.
//! The `default_included` per-entity flag is encoded as a sentinel row
//! (`rule_type='role'`, `rule_value='__default__'`) inside the same table;
//! [`AccessControlRepository::list_rules_for_entity`] and
//! [`AccessControlRepository::list_rules_bulk`] filter that sentinel out so
//! callers only see real assignments.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::RuleId;

use super::error::{AuthzError, AuthzResult};
use super::types::{Access, AccessRule, EntityKind, RuleType};

const DEFAULT_SENTINEL_VALUE: &str = "__default__";

#[derive(Debug, Clone)]
pub struct ExportRuleRow {
    pub entity_type: String,
    pub entity_id: String,
    pub rule_type: String,
    pub rule_value: String,
    pub access: String,
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct UpsertRuleParams<'a> {
    pub entity_type: EntityKind,
    pub entity_id: &'a str,
    pub rule_type: RuleType,
    pub rule_value: &'a str,
    pub access: Access,
    /// Operator-supplied note explaining *why* this rule exists.
    /// Surfaced in the matrix tooltip and in the audit row's
    /// `evaluated_rules` JSON when the rule decides. `None` means
    /// the operator declined to give a reason.
    pub justification: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct AccessControlRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl AccessControlRepository {
    pub fn new(db: &DbPool) -> AuthzResult<Self> {
        let pool = db
            .pool_arc()
            .map_err(|err| AuthzError::Validation(err.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|err| AuthzError::Validation(err.to_string()))?;
        Ok(Self { pool, write_pool })
    }

    pub fn from_pool(pool: Arc<PgPool>) -> Self {
        let write_pool = Arc::clone(&pool);
        Self { pool, write_pool }
    }

    pub async fn list_role_department_rules_for_export(&self) -> AuthzResult<Vec<ExportRuleRow>> {
        let rows = sqlx::query_as!(
            ExportRuleRow,
            r#"
            SELECT entity_type, entity_id, rule_type, rule_value, access, justification
            FROM access_control_rules
            WHERE rule_type IN ('role', 'department')
              AND rule_value <> '__default__'
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
            SELECT id, rule_type, rule_value, access, default_included, justification
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
            if is_sentinel(&row.rule_type, &row.rule_value) {
                continue;
            }
            out.push(AccessRule {
                id: RuleId::new(row.id),
                rule_type: RuleType::from_str(&row.rule_type)?,
                rule_value: row.rule_value,
                access: Access::from_str(&row.access)?,
                default_included: row.default_included,
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
            SELECT entity_id, id, rule_type, rule_value, access, default_included, justification
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
            if is_sentinel(&row.rule_type, &row.rule_value) {
                continue;
            }
            let rule = AccessRule {
                id: RuleId::new(row.id),
                rule_type: RuleType::from_str(&row.rule_type)?,
                rule_value: row.rule_value,
                access: Access::from_str(&row.access)?,
                default_included: row.default_included,
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
                (id, entity_type, entity_id, rule_type, rule_value, access, default_included, justification)
            VALUES ($1, $2, $3, $4, $5, $6, false, $7)
            ON CONFLICT (entity_type, entity_id, rule_type, rule_value)
            DO UPDATE SET
                access = EXCLUDED.access,
                justification = COALESCE(EXCLUDED.justification, access_control_rules.justification),
                updated_at = NOW()
            RETURNING id, rule_type, rule_value, access, default_included, justification
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
            rule_type: RuleType::from_str(&row.rule_type)?,
            rule_value: row.rule_value,
            access: Access::from_str(&row.access)?,
            default_included: row.default_included,
            justification: row.justification,
        })
    }

    /// Update only the justification on an existing rule. Pass `None` to
    /// clear the operator note.
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

    pub async fn set_default_included(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
        value: bool,
    ) -> AuthzResult<()> {
        if value {
            let id = RuleId::generate();
            sqlx::query!(
                r#"
                INSERT INTO access_control_rules
                    (id, entity_type, entity_id, rule_type, rule_value, access, default_included)
                VALUES ($1, $2, $3, 'role', $4, 'allow', true)
                ON CONFLICT (entity_type, entity_id, rule_type, rule_value)
                DO UPDATE SET default_included = true, updated_at = NOW()
                "#,
                id.as_str(),
                entity_type.as_str(),
                entity_id,
                DEFAULT_SENTINEL_VALUE,
            )
            .execute(&*self.write_pool)
            .await?;
        } else {
            sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE entity_type = $1
                  AND entity_id = $2
                  AND rule_type = 'role'
                  AND rule_value = $3
                "#,
                entity_type.as_str(),
                entity_id,
                DEFAULT_SENTINEL_VALUE,
            )
            .execute(&*self.write_pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_default_included(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
    ) -> AuthzResult<bool> {
        let row = sqlx::query!(
            r#"
            SELECT default_included FROM access_control_rules
            WHERE entity_type = $1
              AND entity_id = $2
              AND rule_type = 'role'
              AND rule_value = $3
            "#,
            entity_type.as_str(),
            entity_id,
            DEFAULT_SENTINEL_VALUE,
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.is_some_and(|r| r.default_included))
    }
}

fn is_sentinel(rule_type: &str, rule_value: &str) -> bool {
    rule_type == "role" && rule_value == DEFAULT_SENTINEL_VALUE
}
