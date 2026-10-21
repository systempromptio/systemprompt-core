//! Trusted source capture shared by synchronization and retained-state
//! contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    GitCheckout, GitSyncRequest, GitSyncResult, ManagedError, ManagedRepository, ManagedSourceId,
    Result, RevisionFiles, UserId, import_tree, resolve_ref,
};
use std::path::{Path, PathBuf};
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

/// Captures over the network with the sandboxed `git` subprocess, checking
/// out under a caller-supplied scratch root so a read-only root filesystem
/// needs no writable `/tmp`.
#[derive(Debug, Clone)]
pub struct NativeGitSourceCapture {
    scratch_root: PathBuf,
    certificate_authority: Option<Vec<u8>>,
}

impl NativeGitSourceCapture {
    #[must_use]
    pub const fn new(scratch_root: PathBuf) -> Self {
        Self {
            scratch_root,
            certificate_authority: None,
        }
    }

    #[must_use]
    pub fn with_certificate_authority(mut self, pem: Vec<u8>) -> Self {
        self.certificate_authority = Some(pem);
        self
    }

    fn capture_into(
        &self,
        temp: &Path,
        request: &GitCaptureRequest<'_>,
    ) -> Result<CapturedGitSource> {
        let GitCaptureRequest {
            repository,
            reference,
            subdirectory,
            root,
            credential,
        } = *request;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let certificate_authority = self
            .certificate_authority
            .as_deref()
            .map(|pem| write_certificate_authority(temp, pem))
            .transpose()?;
        let commit = resolve_ref(
            repository,
            reference,
            credential,
            certificate_authority.as_deref(),
            deadline,
        )?;
        let files = import_tree(&GitCheckout {
            temp,
            repository,
            commit: &commit,
            subdirectory,
            root,
            credential,
            certificate_authority: certificate_authority.as_deref(),
            deadline,
        })?;
        Ok(CapturedGitSource { commit, files })
    }
}

fn write_certificate_authority(temp: &Path, certificate: &[u8]) -> Result<PathBuf> {
    use std::io::Write;
    let path = temp.join(".systemprompt-ca.pem");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path)?.write_all(certificate)?;
    Ok(path)
}

impl GitSourceCapture for NativeGitSourceCapture {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        std::fs::create_dir_all(&self.scratch_root)?;
        let temp = self.scratch_root.join(format!(
            "systemprompt-managed-{}",
            ManagedSourceId::generate()
        ));
        super::super::git_execution::create_private_directory(&temp)?;
        let captured = self.capture_into(&temp, request);
        std::fs::remove_dir_all(&temp)?;
        if temp.exists() {
            return Err(ManagedError::Integrity);
        }
        captured
    }
}
