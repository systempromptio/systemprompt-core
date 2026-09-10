//! Row-level upsert primitives shared by the config and marketplace
//! ingestion passes.
//!
//! A [`Target`] is one resolved `(entity, rule_type, rule_value, access)`
//! tuple. [`upsert_entity_row`] / [`upsert_marketplace_entity_row`] satisfy the
//! `access_control_rules` FK and carry the authoritative `default_included`
//! flag; [`upsert_target`] performs the idempotent insert-or-update and reports
//! the [`UpsertOutcome`].
//!
//! Every rule row carries the provenance of the pass that wrote it. A row
//! stamped [`DASHBOARD_SOURCE`] was authored by an operator through the admin
//! surface and outranks the file that ingestion is projecting: ingestion
//! reports it as [`UpsertOutcome::Protected`] and leaves it alone, whatever
//! `override_existing` says.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RuleId;

use crate::authz::error::AuthzResult;
use crate::authz::types::{EntityKind, RuleType};

pub(super) const SOURCE_LABEL: &str = "ingestion:access_control_config";

pub const DASHBOARD_SOURCE: &str = "dashboard";

pub const YAML_SOURCE: &str = "yaml";

#[derive(Debug)]
pub(super) struct Target<'a> {
    pub(super) entity_kind: EntityKind,
    pub(super) entity_id: &'a str,
    pub(super) rule_type: RuleType,
    pub(super) rule_value: &'a str,
    pub(super) access: &'static str,
    pub(super) justification: Option<&'a str>,
    pub(super) source: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UpsertOutcome {
    Inserted,
    Updated,
    Skipped,
    Protected,
}

pub(super) async fn upsert_entity_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn upsert_marketplace_entity_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: &str,
    default_included: bool,
) -> AuthzResult<()> {
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
        SELECT id, access, justification, source
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
        if row.source == DASHBOARD_SOURCE {
            return Ok(UpsertOutcome::Protected);
        }
        if !override_existing {
            return Ok(UpsertOutcome::Skipped);
        }
        // Why: a row whose content matches but whose source does not is a grant
        // that changed hands. Leaving the old source on it strands the row —
        // the bundle that now declares it will not prune it, and the bundle
        // that used to may never run again.
        let unchanged = row.access == target.access
            && row.justification.as_deref() == target.justification
            && row.source == target.source;
        if unchanged {
            return Ok(UpsertOutcome::Skipped);
        }
        sqlx::query!(
            r#"
            UPDATE access_control_rules
            SET access = $2,
                justification = $3,
                source = $4,
                updated_at = NOW()
            WHERE id = $1
            "#,
            row.id,
            target.access,
            target.justification,
            target.source,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Updated)
    } else {
        let id = RuleId::generate();
        sqlx::query!(
            r#"
            INSERT INTO access_control_rules
                (id, entity_type, entity_id, rule_type, rule_value, access, justification, source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            id.as_str(),
            target.entity_kind.as_str(),
            target.entity_id,
            target.rule_type.to_string(),
            target.rule_value,
            target.access,
            target.justification,
            target.source,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Inserted)
    }
}
