//! The access-control plane's reads and writes, on the applier's transaction.
//!
//! A plane entity is one `access_control_entities` row with its non-user
//! `access_control_rules` rows. Writing an entity replaces exactly those rule
//! rows and the entity's `default_included`; `rule_type = 'user'` rows are
//! runtime state and are never read or touched here. Removing a declaration
//! deletes its rules; a pattern or entitlement entity (a glob, a `model`, a
//! `provider`) is then deleted with its row unless a per-user rule still
//! hangs off it, while a catalog entity keeps its row and closes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;


use crate::authz::error::AuthzResult;
use crate::authz::rules_file::{CanonicalEntity, CanonicalRule, EntityTarget};
use crate::authz::types::{Access, EntityKind};

pub(super) use systemprompt_database::PlaneTransaction;

pub(super) async fn list_declared_access(
    tx: &mut PlaneTransaction,
) -> AuthzResult<BTreeMap<String, CanonicalEntity>> {
    let rows = sqlx::query!(
        r#"SELECT e.entity_type, e.entity_id, e.default_included, r.rule_type, r.rule_value,
                  r.access AS "access: Access", r.justification
             FROM access_control_entities e
             JOIN access_control_rules r
               ON r.entity_type = e.entity_type AND r.entity_id = e.entity_id
            WHERE r.rule_type <> 'user'"#
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut out: BTreeMap<String, CanonicalEntity> = BTreeMap::new();
    for row in rows {
        let entity = out
            .entry(format!("{}/{}", row.entity_type, row.entity_id))
            .or_default();
        entity.default_included = row.default_included;
        entity.rules.insert(CanonicalRule {
            rule_type: row.rule_type,
            rule_value: row.rule_value,
            access: row.access,
            why: row.justification.unwrap_or_default(),
        });
    }
    Ok(out)
}

pub(super) async fn set_entity(
    tx: &mut PlaneTransaction,
    target: &EntityTarget,
    canonical: &CanonicalEntity,
    source: &str,
) -> AuthzResult<()> {
    let kind = target.kind.as_str();
    sqlx::query!(
        r"INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
          VALUES ($1, $2, $3, $4)
          ON CONFLICT (entity_type, entity_id) DO UPDATE
             SET default_included = EXCLUDED.default_included,
                 source = EXCLUDED.source,
                 updated_at = NOW()",
        kind,
        target.id,
        canonical.default_included,
        source,
    )
    .execute(&mut **tx)
    .await?;
    delete_declared_rules(tx, target).await?;
    let rules: Vec<&CanonicalRule> = canonical.rules.iter().collect();
    let rule_types: Vec<String> = rules.iter().map(|r| r.rule_type.clone()).collect();
    let values: Vec<String> = rules.iter().map(|r| r.rule_value.clone()).collect();
    let access: Vec<String> = rules.iter().map(|r| r.access.to_string()).collect();
    let whys: Vec<String> = rules.iter().map(|r| r.why.clone()).collect();
    sqlx::query!(
        r"INSERT INTO access_control_rules
              (entity_type, entity_id, rule_type, rule_value, access, justification, source)
          SELECT $1, $2, t.rule_type, t.rule_value, t.access, NULLIF(t.why, ''), $3
            FROM UNNEST($4::text[], $5::text[], $6::text[], $7::text[])
                 AS t(rule_type, rule_value, access, why)",
        kind,
        target.id,
        source,
        &rule_types,
        &values,
        &access,
        &whys,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn clear_entity(
    tx: &mut PlaneTransaction,
    target: &EntityTarget,
) -> AuthzResult<()> {
    delete_declared_rules(tx, target).await?;
    let kind = target.kind.as_str();
    let declared_only =
        target.is_glob() || matches!(target.kind, EntityKind::Model | EntityKind::Provider);
    if declared_only {
        sqlx::query!(
            r"DELETE FROM access_control_entities e
               WHERE e.entity_type = $1 AND e.entity_id = $2
                 AND NOT EXISTS (SELECT 1 FROM access_control_rules r
                                  WHERE r.entity_type = e.entity_type
                                    AND r.entity_id = e.entity_id)",
            kind,
            target.id,
        )
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    sqlx::query!(
        r"UPDATE access_control_entities SET default_included = false, updated_at = NOW()
           WHERE entity_type = $1 AND entity_id = $2",
        kind,
        target.id,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn delete_declared_rules(
    tx: &mut PlaneTransaction,
    target: &EntityTarget,
) -> AuthzResult<()> {
    sqlx::query!(
        r"DELETE FROM access_control_rules
           WHERE entity_type = $1 AND entity_id = $2 AND rule_type <> 'user'",
        target.kind.as_str(),
        target.id,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
