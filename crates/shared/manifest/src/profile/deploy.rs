//! How a self-managed tenant is deployed and how its operators sign in.
//!
//! `cloud.deploy` names the image registry the CLI pushes to and the `GitOps`
//! working copy whose values file pins the pushed image digest; the cluster's
//! own controller rolls the change out. `cloud.operator` names the OIDC issuer
//! operators sign in against before the CLI exchanges that identity for a
//! tenant session. Neither applies to the managed `systemprompt` provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_GITOPS_COMMIT_MESSAGE: &str = "deploy({tenant}): {digest}";

/// Registry and `GitOps` target for `cloud deploy` on a self-managed provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeployConfig {
    pub registry: String,

    pub repository: String,

    pub gitops: GitOpsConfig,
}

impl DeployConfig {
    #[must_use]
    pub fn image_repository(&self) -> String {
        format!(
            "{}/{}",
            self.registry.trim_end_matches('/'),
            self.repository.trim_matches('/')
        )
    }
}

/// The ops-repository working copy and the values-file key that carries the
/// tenant's image digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GitOpsConfig {
    pub repo_path: PathBuf,

    pub values_file: PathBuf,

    pub image_digest_key: String,

    pub branch: String,

    #[serde(default = "default_commit_message")]
    pub commit_message: String,

    #[serde(default)]
    pub push: bool,
}

impl GitOpsConfig {
    #[must_use]
    pub fn digest_key_segments(&self) -> Vec<&str> {
        self.image_digest_key.split('.').collect()
    }

    #[must_use]
    pub fn render_commit_message(&self, tenant: &str, digest: &str) -> String {
        const TENANT: &str = concat!("{", "tenant", "}");
        const DIGEST: &str = concat!("{", "digest", "}");
        self.commit_message
            .replace(TENANT, tenant)
            .replace(DIGEST, digest)
    }
}

fn default_commit_message() -> String {
    DEFAULT_GITOPS_COMMIT_MESSAGE.to_owned()
}

/// The OIDC issuer operators sign in against for self-managed tenants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperatorAuthConfig {
    pub issuer: String,

    pub client_id: String,

    #[serde(default = "default_operator_scopes")]
    pub scopes: Vec<String>,
}

#[must_use]
pub fn default_operator_scopes() -> Vec<String> {
    ["openid", "email", "profile"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}
