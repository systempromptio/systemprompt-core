//! Bootstrap-time projection of [`AccessControlConfig`] into the two-table
//! authz schema (`access_control_entities` + `access_control_rules`).
//!
//! This is the sanctioned YAML → DB ingestion path for authorization rules.
//! Direction is fixed (YAML → DB). Per-user overrides (`rule_type='user'`) are
//! runtime state and are *never* touched here, regardless of `delete_orphans`.
//!
//! Each rule's target is resolved before any write: a literal `entity_id` maps
//! to itself; an `entity_match` glob is expanded against the entities already
//! in the catalog for that kind (see [`super::config::RuleTarget`]). Every
//! resolved id is upserted into `access_control_entities` carrying the rule's
//! `default_included` flag — so the FK on `access_control_rules` is satisfied
//! and the resolver never sees the entity as `UnknownEntity`. For a kind the
//! caller enforces through [`RegisteredEntities`], a literal id outside the
//! registered set is rejected before any write instead of materialised.
//! Nothing is written when that check fails: resolution runs inside the
//! transaction but before every write, so the error rolls back an empty one.
//!
//! # Ownership
//!
//! Every rule row records the pass that wrote it in `access_control_rules
//! .source` — `yaml` for the baked services tree, `bundle:<name>` for a
//! fetched services bundle, `dashboard` for an operator edit. Three rules
//! follow from that column:
//!
//! * a `dashboard` row is never updated and never pruned by ingestion; it is
//!   reported as [`IngestReport::protected`] and left exactly as the operator
//!   left it;
//! * `delete_orphans` prunes only rows whose `source` is this pass's own and
//!   whose entity falls inside the caller's [`IngestScope`], so one bundle
//!   cannot revoke another's grants;
//! * `rule_type = 'user'` rows are runtime state and stay untouched everywhere,
//!   as before.
//!
//! After the writes, every `role` value the pass mentioned is checked against
//! the users table and the ones nobody holds are warned about and returned in
//! [`IngestReport::unknown_subjects`]. Only the role dimension is checkable in
//! core: roles live in the users table, while group and project dimensions are
//! extension-owned and have no core table to check against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
pub mod glob;
mod marketplace;
mod messaging;
mod resolve;
mod scope;
mod subjects;
mod upsert;

use std::collections::BTreeSet;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

use super::config::AccessControlConfig;
use super::error::{AuthzError, AuthzResult};
use super::types::RuleType;

pub use catalog::RegisteredEntities;
use resolve::{prune_role_rules, resolve_rules};
pub use scope::IngestScope;
pub use subjects::UnknownSubject;
use subjects::{SubjectMention, find_unknown_subjects};
pub use upsert::{DASHBOARD_SOURCE, YAML_SOURCE};
use upsert::{SOURCE_LABEL, Target, UpsertOutcome, upsert_entity_row, upsert_target};

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub override_existing: bool,
    pub delete_orphans: bool,
    pub source: String,
    pub scope: IngestScope,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            override_existing: false,
            delete_orphans: false,
            source: YAML_SOURCE.to_owned(),
            scope: IngestScope::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IngestReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub protected: usize,
    pub unknown_subjects: Vec<UnknownSubject>,
}

const fn tally(report: &mut IngestReport, outcome: UpsertOutcome) {
    match outcome {
        UpsertOutcome::Inserted => report.inserted += 1,
        UpsertOutcome::Updated => report.updated += 1,
        UpsertOutcome::Skipped => report.skipped += 1,
        UpsertOutcome::Protected => report.protected += 1,
    }
}

#[derive(Debug, Clone)]
pub struct AccessControlIngestionService {
    write_pool: Arc<PgPool>,
}

impl AccessControlIngestionService {
    pub fn new(db: &DbPool) -> AuthzResult<Self> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|err| AuthzError::Validation(err.to_string()))?;
        Ok(Self { write_pool })
    }

    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { write_pool: pool }
    }

    pub async fn ingest_config_from_yaml_path(
        &self,
        yaml_path: &std::path::Path,
        options: IngestOptions,
        registered: &RegisteredEntities,
    ) -> AuthzResult<IngestReport> {
        let raw = std::fs::read_to_string(yaml_path).map_err(|err| {
            AuthzError::Validation(format!("failed to read {}: {err}", yaml_path.display()))
        })?;
        let cfg: AccessControlConfig = serde_yaml::from_str(&raw).map_err(|err| {
            AuthzError::Validation(format!(
                "failed to parse {} as AccessControlConfig: {err}",
                yaml_path.display()
            ))
        })?;
        self.ingest_config(&cfg, options, registered).await
    }

    pub async fn ingest_config(
        &self,
        cfg: &AccessControlConfig,
        options: IngestOptions,
        registered: &RegisteredEntities,
    ) -> AuthzResult<IngestReport> {
        cfg.validate()?;

        let mut tx = self.write_pool.begin().await?;
        let validated = resolve_rules(&mut tx, &cfg.rules, registered).await?;
        let resolved = validated.rules();
        let mut report = IngestReport::default();

        if options.delete_orphans {
            report.deleted = prune_role_rules(&mut tx, resolved, &options).await?;
        }

        let mut mentions = BTreeSet::new();
        for rule in resolved {
            for id in &rule.ids {
                upsert_entity_row(
                    &mut tx,
                    rule.entity_kind,
                    id,
                    rule.default_included,
                    SOURCE_LABEL,
                )
                .await?;
                for role in rule.roles {
                    let target = Target {
                        entity_kind: rule.entity_kind,
                        entity_id: id,
                        rule_type: RuleType::ROLE,
                        rule_value: role,
                        access: rule.access,
                        justification: rule.justification,
                        source: &options.source,
                    };
                    let outcome =
                        upsert_target(&mut tx, &target, options.override_existing).await?;
                    tally(&mut report, outcome);
                    mentions.insert(SubjectMention {
                        rule_type: RuleType::ROLE.to_string(),
                        value: role.clone(),
                        entity: format!("{}:{id}", rule.entity_kind.as_str()),
                    });
                }
            }
        }

        report.unknown_subjects = find_unknown_subjects(&mut tx, &mentions).await?;

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_access_control_loaded",
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            protected = report.protected,
            unknown_subjects = report.unknown_subjects.len(),
            source = %options.source,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "access-control YAML ingested",
        );

        Ok(report)
    }
}
