//! Projection of the services tree's own access declarations into the
//! two-table authz schema: messaging app seeds.
//!
//! `services/access-control/rules.yaml` and marketplace `access` blocks are not
//! ingested here; they are the `access_control` config plane, applied
//! explicitly by `config apply`.
//!
//! # Ownership
//!
//! Every rule row records the pass that wrote it in `access_control_rules
//! .source`, in the closed vocabulary (`code:<file>` for the baked tree,
//! `bundle:<name>` for a fetched services bundle, `console:<user>` for an
//! operator edit, `migration`). Three rules follow from that column:
//!
//! * a `console:` row is never updated and never pruned by ingestion; it is
//!   reported as [`IngestReport::protected`] and left exactly as the operator
//!   left it;
//! * `delete_orphans` prunes only rows whose `source` is this pass's own and
//!   whose entity falls inside the caller's [`IngestScope`], so one bundle
//!   cannot revoke another's grants;
//! * `rule_type = 'user'` rows are runtime state and stay untouched everywhere.
//!
//! After the writes, every `role` value the pass mentioned is checked against
//! the users table and the ones nobody holds are warned about and returned in
//! [`IngestReport::unknown_subjects`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
pub mod glob;
mod messaging;
mod scope;
mod subjects;
mod upsert;

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

use super::subject_directory::{RoleDirectory, SharedRoleDirectory, discover_role_directory};

pub use catalog::RegisteredEntities;
pub use scope::IngestScope;
pub use subjects::UnknownSubject;
use upsert::UpsertOutcome;
pub use upsert::{CONSOLE_SOURCE_PREFIX, SERVICES_SOURCE};

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
            source: SERVICES_SOURCE.to_owned(),
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
    role_directory: Option<SharedRoleDirectory>,
}

impl AccessControlIngestionService {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self::from_pool(write_pool)
    }

    pub fn from_pool(pool: Arc<PgPool>) -> Self {
        let role_directory = discover_role_directory(&pool);
        Self {
            write_pool: pool,
            role_directory,
        }
    }

    fn role_directory(&self) -> Option<&dyn RoleDirectory> {
        self.role_directory.as_deref()
    }
}
