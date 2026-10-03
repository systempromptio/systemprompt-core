//! Transaction-scoped queries behind access-control ingestion.
//!
//! Every function runs on the caller's connection so a whole ingestion pass
//! commits or rolls back as one transaction; the insert-or-update decision and
//! the `dashboard` protection rule stay in the ingestion service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::RuleId;

use crate::authz::error::AuthzResult;
use crate::authz::types::{EntityKind, RuleType};

#[derive(Debug)]
pub(crate) struct IngestRule<'a> {
    pub entity_kind: EntityKind,
    pub entity_id: &'a str,
    pub rule_type: RuleType,
    pub rule_value: &'a str,
    pub access: &'static str,
    pub justification: Option<&'a str>,
    pub source: &'a str,
}

#[derive(Debug)]
pub(crate) struct StoredRule {
    pub id: RuleId,
    pub access: String,
    pub justification: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IngestionRepository;

impl IngestionRepository {
    pub(crate) async fn upsert_entity(
        conn: &mut PgConnection,
        entity_kind: EntityKind,
        entity_id: &str,
        default_included: bool,
        source: &str,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
        INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET default_included = EXCLUDED.default_included,
                      source = EXCLUDED.source,
                      updated_at = NOW()
        "#,
            entity_kind.as_str(),
            entity_id,
            default_included,
            source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub(crate) async fn upsert_marketplace_entity(
        conn: &mut PgConnection,
        entity_id: &str,
        default_included: bool,
        source: &str,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
        INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
        VALUES ('marketplace', $1, $2, $3)
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET default_included = EXCLUDED.default_included,
                      source = EXCLUDED.source
        "#,
            entity_id,
            default_included,
            source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub(crate) async fn find_rule(
        conn: &mut PgConnection,
        rule: &IngestRule<'_>,
    ) -> AuthzResult<Option<StoredRule>> {
        let row = sqlx::query!(
            r#"
        SELECT id, access, justification, source
        FROM access_control_rules
        WHERE entity_type = $1 AND entity_id = $2
          AND rule_type = $3 AND rule_value = $4
        "#,
            rule.entity_kind.as_str(),
            rule.entity_id,
            rule.rule_type.to_string(),
            rule.rule_value,
        )
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|row| StoredRule {
            id: RuleId::new(row.id),
            access: row.access,
            justification: row.justification,
            source: row.source,
        }))
    }

    pub(crate) async fn update_rule(
        conn: &mut PgConnection,
        id: &RuleId,
        rule: &IngestRule<'_>,
    ) -> AuthzResult<()> {
        sqlx::query!(
            r#"
            UPDATE access_control_rules
            SET access = $2,
                justification = $3,
                source = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
            id.as_str(),
            rule.access,
            rule.justification,
            rule.source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub(crate) async fn insert_rule(
        conn: &mut PgConnection,
        rule: &IngestRule<'_>,
    ) -> AuthzResult<()> {
        let id = RuleId::generate();
        sqlx::query!(
            r#"
            INSERT INTO access_control_rules
                (id, entity_type, entity_id, rule_type, rule_value, access, justification, source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            id.as_str(),
            rule.entity_kind.as_str(),
            rule.entity_id,
            rule.rule_type.to_string(),
            rule.rule_value,
            rule.access,
            rule.justification,
            rule.source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    pub(crate) async fn list_entity_ids(
        conn: &mut PgConnection,
        kind: EntityKind,
    ) -> AuthzResult<Vec<String>> {
        let rows = sqlx::query!(
            r#"
        SELECT entity_id
        FROM access_control_entities
        WHERE entity_type = $1
        "#,
            kind.as_str(),
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|row| row.entity_id).collect())
    }

    pub(crate) async fn delete_role_rules_for(
        conn: &mut PgConnection,
        entity_types: &[String],
        entity_ids: &[String],
        source: &str,
    ) -> AuthzResult<u64> {
        let res = sqlx::query!(
            r#"
        DELETE FROM access_control_rules
        WHERE rule_type = 'role'
          AND source = $3
          AND (entity_type, entity_id) IN (
              SELECT * FROM UNNEST($1::text[], $2::text[])
          )
        "#,
            entity_types,
            entity_ids,
            source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(res.rows_affected())
    }

    pub(crate) async fn delete_marketplace_bands(
        conn: &mut PgConnection,
        entity_ids: &[String],
        rule_types: &[String],
        source: &str,
    ) -> AuthzResult<u64> {
        let res = sqlx::query!(
            r#"
        DELETE FROM access_control_rules
        WHERE entity_type = 'marketplace'
          AND source = $3
          AND (entity_id, rule_type) IN (
              SELECT * FROM UNNEST($1::text[], $2::text[])
          )
        "#,
            entity_ids,
            rule_types,
            source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(res.rows_affected())
    }

    pub(crate) async fn delete_app_role_rules(
        conn: &mut PgConnection,
        kind: EntityKind,
        entity_ids: &[String],
        source: &str,
    ) -> AuthzResult<u64> {
        let res = sqlx::query!(
            r#"
                DELETE FROM access_control_rules
                WHERE rule_type = 'role'
                  AND entity_type = $1
                  AND source = $3
                  AND entity_id = ANY($2::text[])
                "#,
            kind.as_str(),
            entity_ids,
            source,
        )
        .execute(&mut *conn)
        .await?;
        Ok(res.rows_affected())
    }
}
