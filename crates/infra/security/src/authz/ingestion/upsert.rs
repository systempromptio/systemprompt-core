//! Row-level upsert primitives shared by the config and marketplace
//! ingestion passes.
//!
//! A [`Target`] is one resolved `(entity, rule_type, rule_value, access)`
//! tuple. [`expand_targets`] flattens a rule's role list into one target per
//! role; [`upsert_entity_row`] / [`upsert_marketplace_entity_row`] satisfy the
//! `access_control_rules` FK; [`upsert_target`] performs the idempotent
//! insert-or-update and reports the [`UpsertOutcome`].

use systemprompt_identifiers::RuleId;

use crate::authz::config::RuleEntry;
use crate::authz::error::AuthzResult;
use crate::authz::types::{Access, EntityKind, RuleType};

const SOURCE_LABEL: &str = "ingestion:access_control_config";

#[derive(Debug)]
pub(super) struct Target<'a> {
    pub(super) entity_kind: EntityKind,
    pub(super) entity_id: &'a str,
    pub(super) rule_type: RuleType,
    pub(super) rule_value: &'a str,
    pub(super) access: &'static str,
    pub(super) justification: Option<&'a str>,
}

pub(super) fn expand_targets(rules: &[RuleEntry]) -> Vec<Target<'_>> {
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let access_str = match rule.access {
            Access::Allow => "allow",
            Access::Deny => "deny",
        };
        for role in &rule.roles {
            out.push(Target {
                entity_kind: rule.entity_type,
                entity_id: rule.entity_id.as_str(),
                rule_type: RuleType::Role,
                rule_value: role.as_str(),
                access: access_str,
                justification: rule.justification.as_deref(),
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UpsertOutcome {
    Inserted,
    Updated,
    Skipped,
}

pub(super) async fn upsert_entity_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Target<'_>,
) -> AuthzResult<()> {
    // Why: the rule FK requires the entity to exist; we never want to
    // clobber an existing default_included flag set by a higher-priority
    // loader (the publish-pipeline bootstrap pass), so this only inserts
    // missing rows.
    sqlx::query!(
        r#"
        INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
        VALUES ($1, $2, false, $3)
        ON CONFLICT (entity_type, entity_id) DO NOTHING
        "#,
        target.entity_kind.as_str(),
        target.entity_id,
        SOURCE_LABEL,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn upsert_marketplace_entity_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: &str,
    default_included: bool,
) -> AuthzResult<()> {
    // Why: unlike the FK-satisfying stub in `upsert_entity_row`, a marketplace
    // carries an authoritative `default_included` flag from its YAML, so this
    // path owns the column and updates it on conflict.
    let source = format!("marketplace:{entity_id}");
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
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn upsert_target(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Target<'_>,
    override_existing: bool,
) -> AuthzResult<UpsertOutcome> {
    let existing = sqlx::query!(
        r#"
        SELECT id, access, justification
        FROM access_control_rules
        WHERE entity_type = $1 AND entity_id = $2
          AND rule_type = $3 AND rule_value = $4
        "#,
        target.entity_kind.as_str(),
        target.entity_id,
        target.rule_type.to_string(),
        target.rule_value,
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = existing {
        if !override_existing {
            return Ok(UpsertOutcome::Skipped);
        }
        let unchanged =
            row.access == target.access && row.justification.as_deref() == target.justification;
        if unchanged {
            return Ok(UpsertOutcome::Skipped);
        }
        sqlx::query!(
            r#"
            UPDATE access_control_rules
            SET access = $2,
                justification = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            row.id,
            target.access,
            target.justification,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Updated)
    } else {
        let id = RuleId::generate();
        sqlx::query!(
            r#"
            INSERT INTO access_control_rules
                (id, entity_type, entity_id, rule_type, rule_value, access, justification)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            id.as_str(),
            target.entity_kind.as_str(),
            target.entity_id,
            target.rule_type.to_string(),
            target.rule_value,
            target.access,
            target.justification,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Inserted)
    }
}
