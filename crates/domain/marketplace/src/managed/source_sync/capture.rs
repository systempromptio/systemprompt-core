//! Trusted source capture shared by synchronization and retained-state
//! contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    GitCheckout, GitSyncRequest, GitSyncResult, ManagedError, ManagedRepository, ManagedSourceId,
    Result, RevisionFiles, UserId, import_tree, resolve_ref,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct CapturedGitSource {
    pub commit: String,
    pub files: RevisionFiles,
}

#[derive(Clone, Copy)]
pub struct GitCaptureRequest<'a> {
    pub repository: &'a str,
    pub reference: &'a str,
    pub subdirectory: Option<&'a str>,
    pub root: &'a str,
    pub credential: Option<&'a str>,
}

impl std::fmt::Debug for GitCaptureRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitCaptureRequest")
            .field("repository", &self.repository)
            .field("reference", &self.reference)
            .field("subdirectory", &self.subdirectory)
            .field("root", &self.root)
            .field("credential", &self.credential.map(|_| "<redacted>"))
            .finish()
    }
}

pub trait GitSourceCapture: Send + Sync {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource>;
}

pub struct GitSynchronizationService {
    repository: ManagedRepository,
    capture: Arc<dyn GitSourceCapture>,
}

impl std::fmt::Debug for GitSynchronizationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitSynchronizationService")
            .field("repository", &self.repository)
            .finish_non_exhaustive()
    }
}

impl GitSynchronizationService {
    pub fn new(repository: ManagedRepository, capture: Arc<dyn GitSourceCapture>) -> Self {
        Self {
            repository,
            capture,
        }
    }

    pub async fn sync(
        &self,
        owner: &UserId,
        request: &GitSyncRequest,
        credential: Option<&str>,
    ) -> Result<GitSyncResult> {
        self.repository
            .sync_git_source_captured(owner, request, credential, Arc::clone(&self.capture))
            .await
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeGitSourceCapture;

impl GitSourceCapture for NativeGitSourceCapture {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        let GitCaptureRequest {
            repository,
            reference,
            subdirectory,
            root,
            credential,
        } = *request;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let commit = resolve_ref(repository, reference, credential, deadline)?;
        let temp = std::env::temp_dir().join(format!(
            "systemprompt-managed-{}",
            ManagedSourceId::generate()
        ));
        super::super::git_execution::create_private_directory(&temp)?;
        let imported = import_tree(&GitCheckout {
            temp: &temp,
            repository,
            commit: &commit,
            subdirectory,
            root,
            credential,
            certificate_authority: None,
            deadline,
        });
        std::fs::remove_dir_all(&temp)?;
        if temp.exists() {
            return Err(ManagedError::Integrity);
        }
        Ok(CapturedGitSource {
            commit,
            files: imported?,
        })
    }
}
