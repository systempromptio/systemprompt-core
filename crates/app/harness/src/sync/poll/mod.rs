//! One poll of one account: the manifest with the last ETag against the pinned
//! key, its acceptance, the profile and model catalog when due, the bundle
//! files, the apply when the projected content changed, the applied version,
//! the status, and then the skills' runtime files when something is left to
//! deliver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use systemprompt_device::client::{ClientError, CredentialSource, ManifestFetch, ManifestKey, with_jwt};
use crate::extension::ExtensionPolicy;
use crate::projection::{
    ContentDigest, ProjectionInput, SkillPublication, digest, plan, wanted_bundle_files,
};
use sp_protocol::ids::{AccountId, UserId};
use sp_protocol::view::{SyncCounts, SyncStatus};
use sp_protocol::wire::catalog::{ModelCatalog, Profile};
use sp_protocol::wire::manifest::Manifest;
use sp_protocol::{HOST_ID, ManifestVersion};
use tokio::sync::watch;

mod catalog;

use crate::sync::acceptance::accept;
use crate::sync::bundles::BundleCache;
use crate::sync::cadence::Clock;
use crate::sync::error::PollError;
use crate::sync::planner::{Outcome, Planner, Request};
use crate::sync::ports::{Instance, Store};
use crate::sync::runtime::RuntimeFiles;
use crate::sync::target::Target;

/// What one account's poll works over: its id, the store and target, its
/// instance, the clock and the status it publishes.
pub struct Scope<S: Store, T: Target> {
    pub account: AccountId,
    pub store: S,
    pub target: T,
    pub instance: S::Instance,
    pub clock: Arc<dyn Clock>,
    pub status: Arc<watch::Sender<SyncStatus>>,
}

impl<S: Store, T: Target> fmt::Debug for Scope<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

/// One account's poller: what it last fetched, accepted and applied, carried
/// from poll to poll.
pub struct Poller<S: Store, T: Target> {
    pub(crate) scope: Scope<S, T>,
    etag: Option<String>,
    manifest: Option<Arc<Manifest>>,
    pending_version: Option<ManifestVersion>,
    seen_user: Option<UserId>,
    profile: Option<Profile>,
    catalog: Option<ModelCatalog>,
    bundles: BundleCache,
    pushed: Option<ContentDigest>,
    counts: SyncCounts,
    publications: Vec<SkillPublication>,
    captured: bool,
    pub(crate) runtime: RuntimeFiles,
}

impl<S: Store, T: Target> fmt::Debug for Poller<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Poller")
            .field("scope", &self.scope)
            .field("etag", &self.etag)
            .field("pushed", &self.pushed)
            .finish_non_exhaustive()
    }
}

impl<S: Store, T: Target> Poller<S, T> {
    pub fn new(scope: Scope<S, T>, bundles: BundleCache, runtime: RuntimeFiles) -> Self {
        Self {
            scope,
            etag: None,
            manifest: None,
            pending_version: None,
            seen_user: None,
            profile: None,
            catalog: None,
            bundles,
            pushed: None,
            counts: SyncCounts::default(),
            publications: Vec::new(),
            captured: false,
            runtime,
        }
    }

    pub async fn poll(&mut self, request: Request, planner: &mut Planner) -> Outcome {
        let account = self.scope.account.clone();
        let already_synced = matches!(
            &*self.scope.status.borrow(),
            SyncStatus::Synced { account: synced, .. } if synced == &account
        );
        if !already_synced {
            self.scope.status.send_replace(SyncStatus::Syncing {
                account: account.clone(),
            });
        }
        let failure = match self.attempt(request, planner).await {
            Ok(()) => return Outcome::Synced,
            Err(failure) => failure,
        };
        let kind = failure.kind();
        tracing::warn!(account = %account, kind = ?kind, error = %failure, "sync failed");
        self.scope.status.send_replace(SyncStatus::Error {
            account,
            kind,
            at: self.scope.clock.now(),
        });
        if matches!(failure, PollError::KeyChanged) {
            return Outcome::Halted;
        }
        Outcome::Failed {
            kind,
            retry_after: failure.retry_after(),
        }
    }

    async fn attempt(&mut self, request: Request, planner: &mut Planner) -> Result<(), PollError> {
        let source = self.scope.store.credentials(&self.scope.account);
        self.read_manifest(&*source, request).await?;
        if planner.catalog_due(request, Instant::now()) {
            self.read_catalog(&*source).await;
            planner.catalog_read(Instant::now());
        }
        let manifest = self.manifest.clone().ok_or(PollError::NoManifest)?;
        self.apply(&*source, &manifest).await?;
        if let Some(version) = self.pending_version.take() {
            self.scope
                .store
                .record_applied_version(&self.scope.account, &version)?;
        }
        let at = self.scope.clock.now();
        let account = self.scope.account.clone();
        self.scope.status.send_replace(SyncStatus::Synced {
            account,
            at,
            counts: self.counts.clone(),
        });
        if !self.captured {
            let outcome = self
                .runtime
                .capture(&self.scope, &manifest.user_id, &self.publications)
                .await;
            self.captured = outcome.complete();
        }
        Ok(())
    }

    async fn read_manifest(
        &mut self,
        source: &dyn CredentialSource,
        request: Request,
    ) -> Result<(), PollError> {
        let account = &self.scope.account;
        let Some(key) = self.scope.store.pinned_key(account)? else {
            self.offer_served_key().await;
            return Err(PollError::KeyChanged);
        };
        let instance = &self.scope.instance;
        let key_ref: &ManifestKey = &key;
        let etag = self.etag.as_deref();
        let no_cache = request.no_cache;
        let fetched = with_jwt(source, move |jwt| async move {
            instance.manifest(&jwt, key_ref, etag, no_cache).await
        })
        .await;
        match fetched {
            Ok(ManifestFetch::NotModified) => Ok(()),
            Ok(ManifestFetch::Fetched { manifest, etag, .. }) => {
                let expected = self
                    .scope
                    .store
                    .user_id(account)?
                    .or_else(|| self.seen_user.clone());
                let last = self.scope.store.last_applied_version(account)?;
                let version = accept(
                    &manifest,
                    expected.as_ref(),
                    last.as_ref(),
                    self.scope.clock.now(),
                )
                .map_err(PollError::Refused)?;
                self.seen_user = Some(manifest.user_id.clone());
                self.etag = etag;
                self.manifest = Some(Arc::new(*manifest));
                self.pending_version = Some(version);
                Ok(())
            },
            Err(ClientError::KeyChanged { .. }) => {
                self.offer_served_key().await;
                Err(PollError::KeyChanged)
            },
            Err(error) => Err(PollError::Client(error)),
        }
    }

    async fn apply(
        &mut self,
        source: &dyn CredentialSource,
        manifest: &Manifest,
    ) -> Result<(), PollError> {
        let wanted = wanted_bundle_files(manifest, HOST_ID);
        let files = self
            .bundles
            .fetch(&self.scope.instance, source, &wanted)
            .await
            .map_err(PollError::Client)?;
        let instance_key = self.scope.store.instance_key(&self.scope.account)?;
        let input = ProjectionInput {
            manifest,
            profile: self.profile.as_ref(),
            catalog: self.catalog.as_ref(),
            instance_url: self.scope.instance.base(),
            instance_key: &instance_key,
            host: HOST_ID,
            bundle_files: &files,
        };
        let content = digest(&input);
        if self.pushed.as_ref() == Some(&content) {
            return Ok(());
        }
        let projection = plan(&input);
        let applied = projection.applied();
        let publications = projection.publications.clone();
        let report = self
            .scope
            .target
            .apply(projection)
            .await
            .map_err(PollError::Apply)?;
        self.scope
            .target
            .govern(
                &self.scope.account,
                applied,
                ExtensionPolicy::enabled(manifest),
            )
            .map_err(PollError::Apply)?;
        tracing::info!(account = %self.scope.account, digest = %content, "instance content applied");
        self.pushed = Some(content);
        self.counts = report.sync_counts();
        self.publications = publications;
        self.captured = false;
        Ok(())
    }

    pub(crate) fn applied_url(&self) -> &str {
        self.scope.instance.base()
    }

    pub(crate) const fn mark_pending(&mut self) {
        self.captured = false;
    }
}
