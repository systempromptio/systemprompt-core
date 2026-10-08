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

use sqlx::PgConnection;

use crate::authz::error::AuthzResult;
use crate::authz::repository::ingestion::IngestionRepository;
use crate::authz::types::EntityKind;

pub(super) use crate::authz::repository::ingestion::IngestRule as Target;

pub(super) const SOURCE_LABEL: &str = "ingestion:access_control_config";

pub const DASHBOARD_SOURCE: &str = "dashboard";

pub const YAML_SOURCE: &str = "yaml";

#[derive(Debug, Clone, Copy)]
pub(super) enum UpsertOutcome {
    Inserted,
    Updated,
    Skipped,
    Protected,
}

pub(super) async fn upsert_entity_row(
    conn: &mut PgConnection,
    entity_kind: EntityKind,
    entity_id: &str,
    default_included: bool,
    source: &str,
) -> AuthzResult<()> {
    IngestionRepository::upsert_entity(conn, entity_kind, entity_id, default_included, source).await
}

pub(super) async fn upsert_marketplace_entity_row(
    conn: &mut PgConnection,
    entity_id: &str,
    default_included: bool,
) -> AuthzResult<()> {
    let source = format!("marketplace:{entity_id}");
    IngestionRepository::upsert_marketplace_entity(conn, entity_id, default_included, &source).await
}

pub(super) async fn upsert_target(
    conn: &mut PgConnection,
    target: &Target<'_>,
    override_existing: bool,
) -> AuthzResult<UpsertOutcome> {
    let Some(row) = IngestionRepository::find_rule(conn, target).await? else {
        IngestionRepository::insert_rule(conn, target).await?;
        return Ok(UpsertOutcome::Inserted);
    };
    if row.source == DASHBOARD_SOURCE {
        return Ok(UpsertOutcome::Protected);
    }
    if !override_existing {
        return Ok(UpsertOutcome::Skipped);
    }
    let unchanged = row.access == target.access
        && row.justification.as_deref() == target.justification
        && row.source == target.source;
    if unchanged {
        return Ok(UpsertOutcome::Skipped);
    }
    IngestionRepository::update_rule(conn, &row.id, target).await?;
    Ok(UpsertOutcome::Updated)
}
