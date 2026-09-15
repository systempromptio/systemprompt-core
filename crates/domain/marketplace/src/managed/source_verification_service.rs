//! Verification orchestration over an injected Git transport and retained
//! source bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::git_spec;
use super::{GitTreeRead, GitTreeReader, require_matching_files};
use crate::managed::{ManagedError, ManagedRepository, Result};
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_identifiers::{DependencyVerificationId, ManagedSourceId, UserId};
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::verification::{
    DependencyVerificationInput, DependencyVerificationManifest, DependencyVerificationRequest,
    VerifiedRevisionManifest,
};

pub struct GitVerificationService {
    managed: ManagedRepository,
    reader: std::sync::Arc<dyn GitTreeReader>,
}

#[derive(Clone, Copy)]
struct VerificationScope<'a> {
    owner: &'a UserId,
    bundle: &'a crate::managed::RevisionBundle,
    credentials: &'a BTreeMap<ManagedSourceId, String>,
    deadline: std::time::Instant,
}

impl GitVerificationService {
    pub fn new(managed: ManagedRepository, reader: std::sync::Arc<dyn GitTreeReader>) -> Self {
        Self { managed, reader }
    }
    pub async fn verify(
        &self,
        owner: &UserId,
        input: &DependencyVerificationRequest,
        credentials: &BTreeMap<ManagedSourceId, String>,
    ) -> Result<DependencyVerificationManifest> {
        input.validate().map_err(|_error| ManagedError::Integrity)?;
        let bundle = self
            .managed
            .get_revision_bundle(owner, &input.root_revision_id)
            .await?;
        if bundle.revisions.len() != input.revisions.len() {
            return Err(ManagedError::Integrity);
        }
        let mut verified = Vec::new();
        let scope = VerificationScope {
            owner,
            bundle: &bundle,
            credentials,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(120),
        };
        for capture in &input.revisions {
            verified.push(self.verify_revision(&scope, capture).await?);
        }
        let manifest = DependencyVerificationManifest {
            id: DependencyVerificationId::generate(),
            version: 1,
            root_revision_id: input.root_revision_id.clone(),
            bundle_digest: ContentDigest::of(&bundle.canonical_bytes()?),
            revisions: verified,
            verified_at: chrono::Utc::now(),
        };
        manifest
            .validate_complete()
            .map_err(|_error| ManagedError::Integrity)?;
        let retained = self
            .managed
            .retain_git_verification(owner, &manifest)
            .await?;
        let retained_inputs: BTreeMap<_, _> = retained
            .revisions
            .iter()
            .map(|item| (&item.provenance.revision_id, &item.provenance))
            .collect();
        if input.revisions.iter().any(|item| {
            retained_inputs
                .get(&item.revision_id)
                .is_none_or(|retained| *retained != item)
        }) {
            return Err(ManagedError::Conflict(
                "Retained verification uses different dependency provenance".to_owned(),
            ));
        }
        Ok(retained)
    }

    async fn verify_revision(
        &self,
        scope: &VerificationScope<'_>,
        capture: &DependencyVerificationInput,
    ) -> Result<VerifiedRevisionManifest> {
        let VerificationScope {
            owner,
            bundle,
            credentials,
            deadline,
        } = *scope;
        let retained_manifest = bundle
            .revisions
            .get(&capture.revision_id)
            .ok_or(ManagedError::Integrity)?;
        let dependencies: BTreeSet<_> = retained_manifest
            .dependencies
            .values()
            .map(|dependency| &dependency.revision_id)
            .collect();
        if dependencies != capture.dependencies.iter().collect() {
            return Err(ManagedError::Integrity);
        }
        self.managed
            .require_git_verification_binding(owner, capture, &bundle.root, retained_manifest)
            .await?;
        let source = self.managed.get_source(owner, &capture.source_id).await?;
        if matches!(
            &source,
            crate::managed::SourceSpec::Git {
                credential_reference: Some(_),
                ..
            }
        ) && !credentials.contains_key(&capture.source_id)
        {
            return Err(ManagedError::Unavailable);
        }
        let (repository, _, subdirectory) = git_spec(source)?;
        systemprompt_models::net::validate_outbound_url(&repository).map_err(|_error| {
            crate::managed::error::invalid("Git repository is not an allowed outbound target")
        })?;
        let credential = credentials.get(&capture.source_id).cloned();
        let imported_capture = capture.clone();
        let reader = std::sync::Arc::clone(&self.reader);
        let files = tokio::task::spawn_blocking(move || {
            reader.read(&GitTreeRead {
                input: &imported_capture,
                repository: &repository,
                subdirectory: subdirectory.as_deref(),
                credential: credential.as_deref(),
                deadline,
            })
        })
        .await
        .map_err(|_error| ManagedError::Integrity)??;
        let retained = bundle.revision_files(&capture.revision_id)?;
        require_matching_files(&files, &retained)?;
        Ok(VerifiedRevisionManifest {
            provenance: capture.clone(),
            content_digest: ContentDigest::of(&serde_jcs::to_vec(&files)?),
            file_count: u32::try_from(files.0.len()).map_err(|_error| ManagedError::Integrity)?,
            bytes_verified: true,
            modes_verified: true,
        })
    }
}

impl std::fmt::Debug for GitVerificationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitVerificationService")
            .field("managed", &self.managed)
            .finish_non_exhaustive()
    }
}
