//! Idempotent distribution, exact installation receipts, and verified
//! attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, DistributionId, EventOutboxId, InstallationReceiptId,
    InvocationAttributionId, ManagedResourceId, PublicationId, ResourceInvocationId,
    ResourceRevisionId, SessionId, UserId,
};
use systemprompt_models::feedback::receipts::ConsumerReceiptRequest;

use super::{AssetDigest, ManagedError, ManagedRepository, PublicationDecision, Result};

mod repository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionClaim {
    pub id: DistributionId,
    pub outbox_id: EventOutboxId,
    pub publication_id: PublicationId,
    pub generation: i64,
    pub payload: PublicationDecision,
    pub claim_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum DistributionState {
    Claimed,
    Distributed,
    Failed,
}

impl DistributionState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Distributed => "distributed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionStatus {
    pub id: DistributionId,
    pub publication_id: PublicationId,
    pub generation: i64,
    pub status: DistributionState,
    pub claimed_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// What a client attests when it reports an installation or an invocation.
///
/// The session it acted under and the owner it acted for are verified;
/// whatever else the client recorded is retained verbatim as immutable
/// evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientEvidence {
    pub session_id: SessionId,
    pub owner_id: UserId,
    #[serde(flatten)]
    // JSON: client-recorded evidence is retained verbatim, never interpreted
    pub recorded: BTreeMap<String, serde_json::Value>,
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
    pub installation_id: ConsumerInstallationId,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub bundle_digest: AssetDigest,
    pub files: Vec<InstalledFile>,
    pub client_evidence: ClientEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationReceipt {
    pub id: InstallationReceiptId,
    pub installation_id: ConsumerInstallationId,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub bundle_digest: AssetDigest,
    pub installed_manifest: Vec<InstalledFile>,
    // Why: owner-recorded receipts predate device-authenticated evidence and
    // the consumer path writes an empty object here, so a stored value that no
    // longer parses is history, not corruption.
    pub client_evidence: Option<ClientEvidence>,
    pub consumer_id: Option<UserId>,
    pub device_id: Option<DeviceId>,
    pub host: Option<String>,
    pub consumer_evidence: Option<ConsumerReceiptRequest>,
    pub fully_verified: bool,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    Production,
    Fixture,
}

impl TrafficClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Fixture => "fixture",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum AttributionStatus {
    Verified,
    RevisionUnknown,
    Unsupported,
    Historical,
}

impl AttributionStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::RevisionUnknown => "revision_unknown",
            Self::Unsupported => "unsupported",
            Self::Historical => "historical",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationAttribution {
    pub id: InvocationAttributionId,
    pub invocation_id: ResourceInvocationId,
    pub installation_id: Option<ConsumerInstallationId>,
    pub resource_id: Option<ManagedResourceId>,
    pub revision_id: Option<ResourceRevisionId>,
    pub publication_generation: Option<i64>,
    pub traffic_class: TrafficClass,
    pub status: AttributionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationAttributionRequest {
    pub invocation_id: ResourceInvocationId,
    pub installation_id: Option<ConsumerInstallationId>,
    pub resource_key: Option<String>,
    pub resource_revision_id: Option<ResourceRevisionId>,
    pub publication_generation: Option<i64>,
    pub traffic_class: TrafficClass,
    pub authenticated_evidence: ClientEvidence,
}
