//! Sharing: grants, the access answer, and the agent caps of a document or
//! folder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::ids::{DocumentFolderId, DocumentId};

/// The folder a grant is inherited from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InheritedFrom {
    pub folder_id: DocumentFolderId,
    pub name: String,
}

/// Who may see a document, at which level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveGrant {
    pub grant_id: String,
    pub subject_kind: String,
    pub subject_value: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub level: String,
    pub granted_by: String,
    pub granted_at: String,
    #[serde(default)]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(default)]
    pub inherited_from: Option<InheritedFrom>,
}

/// The agent cap summary in an access answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessCap {
    pub cap: String,
    pub policy_cap: String,
    #[serde(default)]
    pub reason: Option<String>,
}

/// `GET /{id}/access`: the read side of sharing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveAccess {
    pub document_id: DocumentId,
    pub owner: String,
    pub my_access: String,
    #[serde(default)]
    pub grants: Vec<DriveGrant>,
    #[serde(default)]
    pub inherited: Vec<DriveGrant>,
    #[serde(default)]
    pub agent_cap: Option<AccessCap>,
}

/// A grant the instance made or changed; its id is what revoking it takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveGrantMade {
    pub id: String,
    pub subject_kind: String,
    pub subject_value: String,
}

/// Where an agent cap comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapSource {
    pub kind: String,
    #[serde(default)]
    pub policy: Option<String>,
}

/// One reading of an agent cap (attended or unattended).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapReading {
    pub agent_cap: String,
    pub policy_cap: String,
    pub source: CapSource,
    #[serde(default)]
    pub reason: Option<String>,
}

/// What an agent may do on a document or folder: `none`, `read`, `suggest` or
/// `edit`. The harness signs in as an attended agent, so `attended.agent_cap`
/// applies to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveAgentCap {
    pub target_kind: String,
    pub target_id: String,
    #[serde(default, rename = "override")]
    pub override_cap: Option<String>,
    pub agent_cap: String,
    pub policy_cap: String,
    pub source: CapSource,
    #[serde(default)]
    pub reason: Option<String>,
    pub attended: CapReading,
    pub unattended: CapReading,
}

/// Body of `POST /api/v1/documents/{id}/grants`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShareRequest {
    pub subject_kind: String,
    pub subject_value: String,
    pub level: String,
}
