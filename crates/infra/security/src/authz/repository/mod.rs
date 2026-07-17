//! `AccessControlRepository` — sqlx-backed access to the two-table authz
//! schema.
//!
//! `access_control_entities` owns one row per `(entity_type, entity_id)` and
//! carries the `default_included` flag plus a `source` provenance string.
//! `access_control_rules` is the per-(entity, subject) grant table, with a
//! foreign key back to the entity catalog. Callers fetch the entity row
//! first (a `None` result signals an entity unknown to access control), then
//! list rules for it, and hand both to [`super::resolver::resolve`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entities;
mod rules;

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

use super::error::{AuthzError, AuthzResult};
use super::types::{Access, EntityKind, RuleType};

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
}
