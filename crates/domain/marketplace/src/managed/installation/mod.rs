//! Idempotent distribution, exact installation receipts, and verified
//! attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    DistributionId, InstallationReceiptId, InvocationAttributionId, ManagedResourceId,
    PublicationId, ResourceRevisionId, UserId,
};

use super::{AssetDigest, ManagedError, ManagedRepository, Result};

mod repository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionClaim {
    pub id: DistributionId,
    pub outbox_id: String,
    pub publication_id: PublicationId,
    pub generation: i64,
    pub payload: serde_json::Value,
    pub claim_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DistributionStatus {
    pub id: String,
    pub publication_id: String,
    pub generation: i64,
    pub status: String,
    pub claimed_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledFile {
    pub revision_id: ResourceRevisionId,
    pub path: String,
    pub digest: AssetDigest,
    pub bytes: u64,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationReceiptRequest {
    pub installation_id: String,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub bundle_digest: AssetDigest,
    pub files: Vec<InstalledFile>,
    pub client_evidence: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationReceipt {
    pub id: InstallationReceiptId,
    pub installation_id: String,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub bundle_digest: AssetDigest,
    pub installed_manifest: Vec<InstalledFile>,
    pub client_evidence: serde_json::Value,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    Production,
    Fixture,
    LiveEvaluation,
    Suggestion,
    Judge,
}

impl TrafficClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Fixture => "fixture",
            Self::LiveEvaluation => "live_evaluation",
            Self::Suggestion => "suggestion",
            Self::Judge => "judge",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationAttribution {
    pub id: InvocationAttributionId,
    pub invocation_id: String,
    pub installation_id: Option<String>,
    pub resource_id: Option<ManagedResourceId>,
    pub revision_id: Option<ResourceRevisionId>,
    pub publication_generation: Option<i64>,
    pub traffic_class: TrafficClass,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationAttributionRequest {
    pub invocation_id: String,
    pub installation_id: Option<String>,
    pub resource_key: Option<String>,
    pub resource_revision_id: Option<ResourceRevisionId>,
    pub publication_generation: Option<i64>,
    pub traffic_class: TrafficClass,
    pub authenticated_evidence: serde_json::Value,
}
