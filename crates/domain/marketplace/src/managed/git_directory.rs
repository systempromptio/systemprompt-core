//! [`GitSourceDirectory`]: the open `DynGitCredentialDirectory` this member
//! provides over its managed sources table and its credential naming rule
//! ([`git_credential_key`]), so the kernel's credential migration reads no
//! marketplace repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_traits::{GitCredentialDirectory, GitCredentialReadError, GitCredentialSource};

use super::ManagedRepository;
use super::git_sources::git_credential_key;

/// The Git source credential read this member provides.
#[derive(Debug, Clone)]
pub struct GitSourceDirectory {
    repository: ManagedRepository,
}

impl GitSourceDirectory {
    #[must_use]
    pub const fn new(repository: ManagedRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl GitCredentialDirectory for GitSourceDirectory {
    async fn sources(&self) -> Result<Vec<GitCredentialSource>, GitCredentialReadError> {
        self.repository
            .git_sources_with_credentials()
            .await
            .map_err(|error| GitCredentialReadError(Box::new(error)))
    }

    fn credential_key(&self, repository: &str, label: &str) -> Option<String> {
        git_credential_key(repository, label)
    }
}
