//! File storage backend selection for a profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SecretName;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    #[default]
    Local,
    Gcs,
}

/// How the `gcs` backend authenticates to Cloud Storage.
///
/// `workload_identity` asks the GCE/GKE metadata server for the token of the
/// service account bound to the workload; `{ secret: NAME }` signs with a
/// service-account key JSON held in the secrets store under `NAME`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "CredentialsRepr", into = "CredentialsRepr")]
pub enum GcsCredentials {
    #[default]
    WorkloadIdentity,
    Secret(SecretName),
}

#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
enum CredentialsRepr {
    Literal(CredentialsLiteral),
    Secret { secret: SecretName },
}

#[derive(Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum CredentialsLiteral {
    WorkloadIdentity,
}

impl From<CredentialsRepr> for GcsCredentials {
    fn from(repr: CredentialsRepr) -> Self {
        match repr {
            CredentialsRepr::Literal(CredentialsLiteral::WorkloadIdentity) => {
                Self::WorkloadIdentity
            },
            CredentialsRepr::Secret { secret } => Self::Secret(secret),
        }
    }
}

impl From<GcsCredentials> for CredentialsRepr {
    fn from(credentials: GcsCredentials) -> Self {
        match credentials {
            GcsCredentials::WorkloadIdentity => Self::Literal(CredentialsLiteral::WorkloadIdentity),
            GcsCredentials::Secret(secret) => Self::Secret { secret },
        }
    }
}

/// Where user-visible files are written and whether the root is shared
/// between replicas.
///
/// `backend: local` writes under `paths.storage`; `shared: true` declares
/// that root is a mount every replica can see, and boot warns when the
/// declaration and the observed mount disagree. `backend: gcs` writes to
/// `bucket` under `prefix`; `bucket`, `prefix`, `public_read` and
/// `credentials` apply to it alone (credentials default to workload
/// identity).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    #[serde(default)]
    pub backend: StorageBackend,
    #[serde(default)]
    pub shared: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default)]
    pub public_read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<GcsCredentials>,
}
