//! `AccessControlRepository` — sqlx-backed access to the two-table authz
//! schema.
//!
//! `access_control_entities` owns one row per `(entity_type, entity_id)` and
//! carries the `default_included` flag plus a `source` provenance string.
//! `access_control_rules` is the per-(entity, subject) grant table, with a
//! foreign key back to the entity catalog. Callers fetch the entity row
//! first (a `None` result signals an entity unknown to access control), then
//! list rules for it, and hand both to [`super::resolver::resolve`].

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::RuleId;

use super::error::{AuthzError, AuthzResult};
use super::types::{Access, AccessRule, EntityKind, EntityRow, RuleType};

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
    /// Operator-supplied note explaining *why* this rule exists. Surfaced in
    /// the matrix tooltip and in the audit row's `evaluated_rules` JSON when
    /// the rule decides. `None` means the operator declined to give a reason.
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

    /// Look up one entity catalog row. `Ok(None)` means the entity has no
    /// catalog row at all (publish-pipeline bootstrap gap) — the resolver
    /// turns this into [`super::DenyReason::UnknownEntity`].
    pub async fn get_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
    ) -> AuthzResult<Option<EntityRow>> {
        let row = sqlx::query!(
            r#"
            SELECT entity_type, entity_id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1 AND entity_id = $2
            "#,
            entity_type.as_str(),
            entity_id,
        )
        .fetch_optional(&*self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(EntityRow {
            kind: EntityKind::from_str(&row.entity_type)?,
            id: row.entity_id,
            default_included: row.default_included,
            source: row.source,
        }))
    }

    /// Upsert an entity catalog row. Always overwrites `default_included` and
    /// `source` so the most recent bootstrap pass wins — the publish pipeline
    /// is the source of truth and runs ahead of YAML grant ingestion.
    pub async fn upsert_entity(
        &self,
        entity_type: EntityKind,
        entity_id: &str,
        default_included: bool,
        source: &str,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (entity_type, entity_id) DO UPDATE
            SET default_included = EXCLUDED.default_included,
                source = EXCLUDED.source,
                updated_at = NOW()
            "#,
            entity_type.as_str(),
            entity_id,
            default_included,
            source,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    /// Bulk-fetch every catalog row for a given kind. Used by the CLI lint and
    /// the publish-pipeline validator to detect rules pointing at entities
    /// the bootstrap pass never registered.
    pub async fn list_entities(&self, entity_type: EntityKind) -> AuthzResult<Vec<EntityRow>> {
        let rows = sqlx::query!(
            r#"
            SELECT entity_type, entity_id, default_included, source
            FROM access_control_entities
            WHERE entity_type = $1
            ORDER BY entity_id
            "#,
            entity_type.as_str(),
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(EntityRow {
                kind: EntityKind::from_str(&row.entity_type)?,
                id: row.entity_id,
                default_included: row.default_included,
                source: row.source,
            });
        }
        Ok(out)
    }

    pub async fn list_role_department_rules_for_export(&self) -> AuthzResult<Vec<ExportRuleRow>> {
        let rows = sqlx::query_as!(
            ExportRuleRow,
            r#"
            SELECT entity_type, entity_id, rule_type, rule_value, access, justification
            FROM access_control_rules
            WHERE rule_type IN ('role', 'department')
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
                rule_type: RuleType::from_str(&row.rule_type)?,
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
                rule_type: RuleType::from_str(&row.rule_type)?,
                rule_value: row.rule_value,
                access: Access::from_str(&row.access)?,
                justification: row.justification,
            };
            out.entry(row.entity_id).or_default().push(rule);
        }
        Ok(out)
    }

    /// Insert or update a grant row. Fails with a foreign-key violation if no
    /// entity catalog row exists — register the entity via
    /// [`Self::upsert_entity`] first.
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
            rule_type: RuleType::from_str(&row.rule_type)?,
            rule_value: row.rule_value,
            access: Access::from_str(&row.access)?,
            justification: row.justification,
        })
    }

    // None clears the operator note.
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
