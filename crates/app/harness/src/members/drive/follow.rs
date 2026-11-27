//! The service that keeps the Drive's local folder sync on the applied
//! account: it follows the handle's applied instance and the Drive's folder
//! preference, and switches the sync to that instance's own folder under the
//! chosen base (the preference, or the product's default under Documents), or
//! stops it while the Drive is withheld or no base folder is known.
//!
//! The composition root holds it as `dyn BackgroundService`, which is why its
//! `run` is an `async_trait` method.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::PathBuf;

use crate::extension::{BackgroundService, CancellationToken, ExtensionSettings};
use async_trait::async_trait;
use systemprompt_device::accounts::Accounts;
use systemprompt_device::client::HttpClient;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_identifiers::DriveDeviceId;

use super::handle::{DriveHandle, Target};
use super::settings::DriveSettings;
use systemprompt_drive::sync::{SyncAccount, account_root};

const SERVICE: &str = "drive-follow";

/// Runs the Drive's local folder sync for as long as the app runs.
///
/// While the handle follows an applied instance and a base folder is known
/// (the `folder` of [`DriveSettings`], else `default_folder`), the sync
/// mirrors that instance into its own subfolder; otherwise it is stopped. A
/// preference that does not decode stops the sync rather than falling back.
/// `run` stops the sync when it is cancelled.
pub struct DriveFollower {
    handle: DriveHandle,
    accounts: Accounts,
    http: HttpClient,
    settings: ExtensionSettings,
    default_folder: Option<PathBuf>,
}

struct Wanted {
    target: Target,
    root: PathBuf,
}

impl Wanted {
    fn key(&self) -> (AccountId, PathBuf) {
        (self.target.account.clone(), self.root.clone())
    }
}

impl DriveFollower {
    pub const fn new(
        handle: DriveHandle,
        accounts: Accounts,
        http: HttpClient,
        settings: ExtensionSettings,
        default_folder: Option<PathBuf>,
    ) -> Self {
        Self {
            handle,
            accounts,
            http,
            settings,
            default_folder,
        }
    }

    fn wanted(&self, target: Option<Target>) -> Option<Wanted> {
        let target = target?;
        let base = self.base_folder()?;
        let root = account_root(&base, target.instance.base_url());
        Some(Wanted { target, root })
    }

    fn base_folder(&self) -> Option<PathBuf> {
        match self.settings.get::<DriveSettings>() {
            Ok(settings) => settings.folder.or_else(|| self.default_folder.clone()),
            Err(error) => {
                tracing::warn!(error = %error, "the Drive folder preference does not decode; the sync stays off");
                None
            },
        }
    }

    async fn switch(&self, wanted: Option<Wanted>) {
        let account = wanted.and_then(|wanted| self.sync_account(wanted));
        let following = account.is_some();
        if let Err(error) = self.handle.sync().switch_account(account).await {
            tracing::warn!(error = %error, "the Drive sync did not start");
            return;
        }
        tracing::info!(following, "the Drive sync follows the applied account");
    }

    fn sync_account(&self, wanted: Wanted) -> Option<SyncAccount> {
        let device_id = match DriveDeviceId::try_new(self.accounts.install_id().into_inner()) {
            Ok(device_id) => device_id,
            Err(error) => {
                tracing::warn!(error = %error, "the install id is not a Drive device id");
                return None;
            },
        };
        Some(SyncAccount {
            credentials: self.accounts.device_tokens(&wanted.target.account),
            http: self.http.clone(),
            root: wanted.root,
            device_id,
            host_name: None,
            instance: wanted.target.instance,
        })
    }
}

#[async_trait]
impl BackgroundService for DriveFollower {
    fn name(&self) -> &'static str {
        SERVICE
    }

    async fn run(&self, cancel: CancellationToken) {
        let mut targets = self.handle.targets();
        let mut settings = self.settings.changes();
        let mut current: Option<(AccountId, PathBuf)> = None;
        loop {
            settings.borrow_and_update();
            let target = targets.borrow_and_update().clone();
            let wanted = self.wanted(target);
            let key = wanted.as_ref().map(Wanted::key);
            if key != current {
                self.switch(wanted).await;
                current = key;
            }
            tokio::select! {
                () = cancel.cancelled() => break,
                changed = targets.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                changed = settings.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
        self.handle.sync().stop().await;
    }
}

impl fmt::Debug for DriveFollower {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DriveFollower")
            .field("handle", &self.handle)
            .field("default_folder", &self.default_folder)
            .finish_non_exhaustive()
    }
}
