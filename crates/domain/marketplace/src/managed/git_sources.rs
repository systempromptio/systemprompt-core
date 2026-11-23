//! Credential resolution for Git source synchronization.
//!
//! A source's `credential_reference` is a label, not a secret name: it resolves
//! only the custom secret `GIT_SOURCE__<HOST>__<LABEL>`, where `<HOST>` is the
//! source repository's lowercased authority. Secrets outside that namespace
//! are unreachable, and a credential stored for one host is never sent to
//! another. [`GitCredentialReferences`] reserves the whole namespace, so no
//! install member may declare or read a name in it.
//!
//! The name uses only `[A-Z0-9_]`, so every secrets source (file, `env`,
//! Vault, cloud secret push) can carry it. Host and label are encoded byte by
//! byte: `a-z` becomes `A-Z`, `0-9` stays, and every other byte becomes `_`
//! plus two upper-case hex digits. An encoded part therefore never contains
//! `__`, the separator, and the encoding is injective over (host, label).
//!
//! A credential still stored under the bare label is not used; the sync fails
//! with [`GitSourceError::CredentialNotMigrated`], which names
//! `admin config secret migrate-git-credentials`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_traits::{SecretReferences, SecretStore};

use super::{GitSyncRequest, GitSyncResult, ManagedError, ManagedRepository, SourceSpec};

#[derive(Debug, thiserror::Error)]
pub enum GitSourceError {
    #[error(transparent)]
    Managed(#[from] ManagedError),
    #[error("Git credential reference is unresolved")]
    CredentialUnresolved,
    #[error(
        "Git credential is stored under its bare label; the runtime resolves only {expected}. \
         Run `admin config secret migrate-git-credentials --apply`"
    )]
    CredentialNotMigrated { expected: String },
    #[error("Operation requires a registered Git source")]
    NotGitSource,
}

#[derive(Debug, Clone)]
pub struct GitSourceOrchestrator {
    managed: ManagedRepository,
    scratch_root: PathBuf,
    secrets: Arc<dyn SecretStore>,
}

impl GitSourceOrchestrator {
    pub const fn new(
        managed: ManagedRepository,
        scratch_root: PathBuf,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            managed,
            scratch_root,
            secrets,
        }
    }

    pub async fn synchronize(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
    ) -> Result<GitSyncResult, GitSourceError> {
        let credential = self.credential(owner, &request.source_id).await?;
        Ok(self
            .managed
            .sync_git_source_with_credential(
                owner,
                request,
                credential.as_deref(),
                &self.scratch_root,
            )
            .await?)
    }

    async fn credential(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
    ) -> Result<Option<String>, GitSourceError> {
        match self.managed.get_source(owner, source).await? {
            SourceSpec::Git {
                repository,
                credential_reference: Some(reference),
                ..
            } => {
                let key = git_credential_key(&repository, &reference)
                    .ok_or(GitSourceError::CredentialUnresolved)?;
                let present = |name: &str| {
                    self.secrets
                        .secret(name)
                        .filter(|value| !value.expose().is_empty())
                };
                match present(&key) {
                    Some(credential) => Ok(Some(credential.expose().to_owned())),
                    None if present(&reference).is_some() => {
                        Err(GitSourceError::CredentialNotMigrated { expected: key })
                    },
                    None => Err(GitSourceError::CredentialUnresolved),
                }
            },
            SourceSpec::Git {
                credential_reference: None,
                ..
            } => Ok(None),
            _ => Err(GitSourceError::NotGitSource),
        }
    }
}

pub fn git_credential_key(repository: &str, reference: &str) -> Option<String> {
    let authority = repository
        .strip_prefix("https://")?
        .split('/')
        .next()
        .filter(|host| !host.is_empty())?
        .to_ascii_lowercase();
    if reference.is_empty() {
        return None;
    }
    Some(format!(
        "{GIT_CREDENTIAL_NAMESPACE}__{}__{}",
        encode_part(&authority),
        encode_part(reference)
    ))
}

fn encode_part(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for byte in part.bytes() {
        match byte {
            b'a'..=b'z' => out.push(char::from(byte.to_ascii_uppercase())),
            b'0'..=b'9' => out.push(char::from(byte)),
            _ => out.push_str(&format!("_{byte:02X}")),
        }
    }
    out
}

pub const GIT_CREDENTIAL_NAMESPACE: &str = "GIT_SOURCE";

pub const MIGRATE_GIT_CREDENTIALS_COMMAND: &str = "admin config secret migrate-git-credentials";

/// Every name in the Git-source credential namespace (`GIT_SOURCE__…`): the
/// managed marketplace reads these, so no install may declare or read one.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitCredentialReferences;

impl GitCredentialReferences {
    #[must_use]
    pub fn shared() -> Arc<dyn SecretReferences> {
        Arc::new(Self)
    }
}

impl SecretReferences for GitCredentialReferences {
    fn references(&self, name: &str) -> bool {
        let prefix = format!("{GIT_CREDENTIAL_NAMESPACE}__");
        name.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
    }
}
