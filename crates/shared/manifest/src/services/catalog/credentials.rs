//! `services/ai/credentials.yaml`: who authenticates and who pays.
//!
//! A credential is the unit of authentication, billing and the Settings UI.
//! A `platform` credential is the deployment's: it names the secret that
//! holds an API key or a Google service-account document, and which of the
//! two it is is read from the secret itself ([`PlatformSecretKind::detect`]).
//! A platform credential may instead declare a `workload_identity`: it names
//! no secret and authenticates as the runtime's ambient Google service
//! account (GKE Workload Identity, Cloud Run, Compute Engine), addressing the
//! Cloud `project` it declares.
//! A `personal` credential is each member's own subscription: it names no
//! secret, every member stores their own key under the credential id, and
//! one member's key is never spent on another's request. A `caller`
//! credential is whatever the client presents on the request itself: it names
//! no secret and stores nothing, the request's own `Authorization` is sent
//! upstream, and it is confined to endpoints the vendor hosts itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CredentialId, SecretName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    Platform,
    Personal,
    Caller,
}

impl CredentialKind {
    #[must_use]
    pub const fn is_personal(self) -> bool {
        matches!(self, Self::Personal)
    }

    #[must_use]
    pub const fn is_caller(self) -> bool {
        matches!(self, Self::Caller)
    }

    #[must_use]
    pub const fn is_platform(self) -> bool {
        matches!(self, Self::Platform)
    }

    #[must_use]
    pub const fn is_member_paid(self) -> bool {
        match self {
            Self::Platform => false,
            Self::Personal | Self::Caller => true,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Personal => "personal",
            Self::Caller => "caller",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CredentialEntry {
    pub kind: CredentialKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<SecretName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity: Option<WorkloadIdentityCredential>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A platform credential backed by the runtime's ambient Google identity.
///
/// `project` is the Cloud project a `{project}` endpoint segment is filled
/// from; the region is part of each endpoint's own URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkloadIdentityCredential {
    pub project: String,
}

impl CredentialEntry {
    #[must_use]
    pub const fn platform(secret: SecretName) -> Self {
        Self {
            kind: CredentialKind::Platform,
            secret: Some(secret),
            workload_identity: None,
            label: None,
        }
    }

    #[must_use]
    pub const fn workload_identity(project: String) -> Self {
        Self {
            kind: CredentialKind::Platform,
            secret: None,
            workload_identity: Some(WorkloadIdentityCredential { project }),
            label: None,
        }
    }

    #[must_use]
    pub const fn personal(label: Option<String>) -> Self {
        Self {
            kind: CredentialKind::Personal,
            secret: None,
            workload_identity: None,
            label,
        }
    }

    #[must_use]
    pub const fn caller(label: Option<String>) -> Self {
        Self {
            kind: CredentialKind::Caller,
            secret: None,
            workload_identity: None,
            label,
        }
    }
}

/// What a platform credential's secret holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformSecretKind {
    ApiKey,
    ServiceAccount,
}

impl PlatformSecretKind {
    #[must_use]
    pub fn detect(secret_value: &str) -> Self {
        let is_service_account =
            match serde_json::from_str::<serde_json::Value>(secret_value.trim()) {
                Ok(doc) => {
                    doc.get("type").and_then(serde_json::Value::as_str) == Some("service_account")
                },
                Err(_not_json) => false,
            };
        if is_service_account {
            Self::ServiceAccount
        } else {
            Self::ApiKey
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct CredentialRegistry {
    pub credentials: BTreeMap<CredentialId, CredentialEntry>,
}

impl CredentialRegistry {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CredentialEntry> {
        self.credentials.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CredentialId, &CredentialEntry)> {
        self.credentials.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}
