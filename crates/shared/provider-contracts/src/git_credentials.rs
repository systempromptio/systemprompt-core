//! [`GitCredentialDirectory`]: the open read of the registered Git sources
//! that name a credential label, and the secret name each label resolves to.
//!
//! The managed-resources member owns the source table and the credential
//! naming rule; a kernel command that edits the secrets file (`admin config
//! secret migrate-git-credentials`) reads both through this seam, never the
//! member's repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;

use crate::doctor::GitCredentialSource;

/// Why the Git source list could not be read.
#[derive(Debug, thiserror::Error)]
#[error("the Git source list could not be read")]
pub struct GitCredentialReadError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

/// The registered Git sources with a credential label, and the secret name
/// the runtime resolves a label to.
///
/// Held as `Arc<dyn GitCredentialDirectory>`, so `#[async_trait]` is
/// required for `dyn` dispatch.
#[async_trait]
pub trait GitCredentialDirectory: Send + Sync {
    async fn sources(&self) -> Result<Vec<GitCredentialSource>, GitCredentialReadError>;

    fn credential_key(&self, repository: &str, label: &str) -> Option<String>;
}

pub type DynGitCredentialDirectory = Arc<dyn GitCredentialDirectory>;
