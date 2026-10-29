//! The background loops the context owns from startup: the notification
//! inbox and drive sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::proxy::ProxyHandle;
use crate::proxy::notifications::{self, InboxHandle, NotificationSignalReceiver};

pub(super) fn spawn_inbox(
    tasks: &crate::tasks::TaskOwner,
    proxy: &ProxyHandle,
    http: &reqwest::Client,
    signals: NotificationSignalReceiver,
) -> Arc<InboxHandle> {
    let (inbox, requests) = InboxHandle::new(notifications::inbox_path());
    if let Some(token_cache) = proxy.token_cache() {
        tasks.spawn(notifications::run_loop(notifications::InboxDeps {
            handle: Arc::clone(&inbox),
            runtime_config: Arc::clone(proxy.runtime_config()),
            token_cache: Arc::clone(token_cache),
            http: http.clone(),
            signals,
            requests,
        }));
    }
    inbox
}

pub(super) fn spawn_drive(
    tasks: &crate::tasks::TaskOwner,
    proxy: &ProxyHandle,
    drive: &Arc<crate::drive::DriveHandle>,
    http: &reqwest::Client,
    signals: crate::proxy::drive_signal::DriveSignalReceiver,
) {
    let Some(token_cache) = proxy.token_cache() else {
        return;
    };
    let disabled = match crate::config::load() {
        Ok(cfg) => cfg.drive.and_then(|drive| drive.enabled) == Some(false),
        Err(error) => {
            tracing::warn!(%error, "drive sync reads no config; it runs with the defaults");
            false
        },
    };
    let Some(root) = crate::drive::DriveRoot::resolve() else {
        tracing::info!("drive sync off: this brand has no workspace folder");
        return;
    };
    if disabled {
        tracing::info!("drive sync off: disabled in the bridge config");
        return;
    }
    let device_id =
        match systemprompt_identifiers::DriveDeviceId::try_new(proxy.install_id().as_str()) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, "drive sync off: the install id is not a device id");
                return;
            },
        };
    tasks.spawn(crate::drive::run::run_loop(crate::drive::run::DriveDeps {
        handle: Arc::clone(drive),
        root,
        runtime_config: Arc::clone(proxy.runtime_config()),
        token_cache: Arc::clone(token_cache),
        http: http.clone(),
        device_id,
        host_name: crate::sysproc::host_name().unwrap_or_else(|| "this device".to_owned()),
        signals,
    }));
}
