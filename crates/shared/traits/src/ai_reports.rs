//! The two AI-store ports kernel commands read: the safety-finding rollup
//! (`logs governance report`) and the catalog-migration writes
//! (`admin config migrate --data`).
//!
//! The `ai` member implements both over its tables and provides each
//! **sealed** (`DynSafetyFindingsReader`, `DynCatalogMigrationPort`): kernel
//! commands read them, a member's command, job or route cannot. Both are
//! held as `Arc<dyn _>`, hence `#[async_trait]` for `dyn` dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::repository::RepositoryError;

/// One group of recorded safety findings: its category, scanner, severity
/// and phase, how many there were, how many blocked, and the latest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyFindingRollup {
    pub category: String,
    pub scanner: String,
    pub severity: String,
    pub phase: String,
    pub count: i64,
    pub blocked_count: i64,
    pub last_seen: DateTime<Utc>,
}

/// Reads the safety-finding rollup, largest groups first.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// `DynSafetyFindingsReader`.
#[async_trait]
pub trait SafetyFindingsReader: Send + Sync {
    async fn rollup(
        &self,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<SafetyFindingRollup>, RepositoryError>;
}

pub type DynSafetyFindingsReader = Arc<dyn SafetyFindingsReader>;

/// One pre-catalog provider entry and the catalog hop it became.
#[derive(Debug, Clone, Copy)]
pub struct LegacyCatalogEntry<'a> {
    pub legacy_provider: &'a str,
    pub provider: &'a str,
    pub endpoint: &'a str,
    pub credential: &'a str,
}

/// The catalog-migration writes; each is idempotent and the backfills are
/// bounded to `batch` rows per call.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// `DynCatalogMigrationPort`.
#[async_trait]
pub trait CatalogMigrationPort: Send + Sync {
    async fn upsert_legacy_map(&self, row: LegacyCatalogEntry<'_>) -> Result<(), RepositoryError>;

    async fn update_personal_keys(
        &self,
        legacy_provider: &str,
        credential: &str,
    ) -> Result<u64, RepositoryError>;

    async fn update_audit_batch(
        &self,
        row: LegacyCatalogEntry<'_>,
        batch: i64,
    ) -> Result<u64, RepositoryError>;

    async fn update_served_audit_batch(
        &self,
        row: LegacyCatalogEntry<'_>,
        routed: &[String],
        batch: i64,
    ) -> Result<u64, RepositoryError>;
}

pub type DynCatalogMigrationPort = Arc<dyn CatalogMigrationPort>;
