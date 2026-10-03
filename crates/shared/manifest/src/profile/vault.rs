//! Profile `secrets.vault:` block — Vault / `OpenBao` KV v2 access.
//!
//! There is deliberately no TLS-verification escape hatch:
//! `deny_unknown_fields` turns a `skip_verify:` key into a parse error rather
//! than a silently downgraded connection. A private CA is configured through
//! `ca_cert_path`.
//!
//! The document at `mount`/`path` holds the whole `secrets.json` shape; `keys`
//! redirects individual entries at a different KV path or field, for identity
//! material shared across instances.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DEFAULT_VAULT_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_VAULT_RETRIES: u8 = 3;
pub const MAX_VAULT_TIMEOUT_SECS: u64 = 120;
pub const MAX_VAULT_RETRIES: u8 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VaultSecretsConfig {
    pub address: String,

    #[serde(default = "default_mount")]
    pub mount: String,

    pub path: String,

    #[serde(default)]
    pub namespace: Option<String>,

    pub auth: VaultAuth,

    #[serde(default)]
    pub keys: BTreeMap<String, VaultKeyRef>,

    #[serde(default)]
    pub ca_cert_path: Option<String>,

    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    #[serde(default = "default_retries")]
    pub retries: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VaultKeyRef {
    pub path: String,

    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "method", rename_all = "lowercase", deny_unknown_fields)]
pub enum VaultAuth {
    Token {
        #[serde(default = "default_token_env")]
        token_env: String,

        #[serde(default)]
        token_file: Option<String>,
    },

    AppRole {
        #[serde(default = "default_role_id_env")]
        role_id_env: String,

        #[serde(default = "default_secret_id_env")]
        secret_id_env: String,

        #[serde(default = "default_approle_mount")]
        mount: String,
    },

    Kubernetes {
        role: String,

        #[serde(default = "default_jwt_path")]
        jwt_path: String,

        #[serde(default = "default_kubernetes_mount")]
        mount: String,
    },
}

impl VaultAuth {
    #[must_use]
    pub const fn method_name(&self) -> &'static str {
        match self {
            Self::Token { .. } => "token",
            Self::AppRole { .. } => "approle",
            Self::Kubernetes { .. } => "kubernetes",
        }
    }
}

fn default_mount() -> String {
    "secret".to_owned()
}

const fn default_timeout_secs() -> u64 {
    DEFAULT_VAULT_TIMEOUT_SECS
}

const fn default_retries() -> u8 {
    DEFAULT_VAULT_RETRIES
}

fn default_token_env() -> String {
    "VAULT_TOKEN".to_owned()
}

fn default_role_id_env() -> String {
    "VAULT_ROLE_ID".to_owned()
}

fn default_secret_id_env() -> String {
    "VAULT_SECRET_ID".to_owned()
}

fn default_approle_mount() -> String {
    "approle".to_owned()
}

fn default_kubernetes_mount() -> String {
    "kubernetes".to_owned()
}

fn default_jwt_path() -> String {
    "/var/run/secrets/kubernetes.io/serviceaccount/token".to_owned()
}
