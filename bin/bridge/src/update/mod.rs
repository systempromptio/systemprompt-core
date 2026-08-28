//! Bridge self-update: ask the gateway what the newest build is, download it,
//! verify it, swap it in place.
//!
//! The gateway is the only update source. Release assets live in a private
//! GitHub repository, so the bridge cannot reach them directly — the gateway
//! holds the token and proxies, which also means staged rollouts and version
//! pinning are gateway-side config rather than a client release.
//!
//! Deliberately not behind the macOS/Windows `gui` gate: the `update` CLI
//! command is the only update path Linux has.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod download;
mod error;
mod install;

pub use download::{DownloadProgress, download_verified, hex_lower};
pub use error::UpdateError;

use crate::gateway::GatewayClient;
use crate::gateway::types::ReleaseManifest;
use systemprompt_identifiers::ValidatedUrl;

#[must_use]
pub fn platform_slug() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("macos"),
        ("windows", _) => Some("windows"),
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateStatus {
    Current {
        version: String,
    },
    Available {
        version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        notes_url: Option<String>,
    },
}

impl UpdateStatus {
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(*self, Self::Available { .. })
    }
}

/// What the GUI shows for the update affordance. Carried on the state snapshot
/// so it rides the existing `state.changed` event.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum UpdateUiState {
    #[default]
    Unknown,
    Current,
    Available {
        version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        notes_url: Option<String>,
    },
    Downloading {
        version: String,
        percent: u8,
    },
    Installing {
        version: String,
    },
    Ready {
        version: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateUiState {
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Available { version, .. }
            | Self::Downloading { version, .. }
            | Self::Installing { version }
            | Self::Ready { version } => Some(version),
            Self::Unknown | Self::Current | Self::Failed { .. } => None,
        }
    }
}

impl From<&UpdateStatus> for UpdateUiState {
    fn from(status: &UpdateStatus) -> Self {
        match status {
            UpdateStatus::Current { .. } => Self::Current,
            UpdateStatus::Available { version, notes_url } => Self::Available {
                version: version.clone(),
                notes_url: notes_url.clone(),
            },
        }
    }
}

pub async fn check(
    client: &GatewayClient,
    bearer: &str,
) -> Result<(UpdateStatus, ReleaseManifest), UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let manifest = client.fetch_latest_release(bearer, platform).await?;
    let status = compare(crate::brand::brand().version, &manifest)?;
    Ok((status, manifest))
}

#[doc(hidden)]
pub fn compare(local: &str, manifest: &ReleaseManifest) -> Result<UpdateStatus, UpdateError> {
    let remote_version = semver::Version::parse(&manifest.version).map_err(|source| {
        UpdateError::BadRemoteVersion {
            version: manifest.version.clone(),
            source,
        }
    })?;
    let local_version =
        semver::Version::parse(local).map_err(|source| UpdateError::BadLocalVersion {
            version: local.to_owned(),
            source,
        })?;
    if remote_version > local_version {
        Ok(UpdateStatus::Available {
            version: manifest.version.clone(),
            notes_url: manifest.notes_url.clone(),
        })
    } else {
        Ok(UpdateStatus::Current {
            version: local.to_owned(),
        })
    }
}

pub async fn apply(
    client: &GatewayClient,
    bearer: &str,
    manifest: &ReleaseManifest,
    on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<std::path::PathBuf, UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let staged = download_verified(client, bearer, platform, manifest, on_progress).await?;
    let installed = install::apply(&staged)?;
    if let Err(e) = std::fs::remove_file(&staged) {
        tracing::debug!(error = %e, path = %staged.display(), "update: staging cleanup failed");
    }
    Ok(installed)
}

pub fn sweep_leftovers() {
    install::sweep_leftovers();
}

pub fn installed_path() -> Result<std::path::PathBuf, UpdateError> {
    install::installed_path()
}

pub fn spawn_installed(installed: &std::path::Path) -> Result<(), UpdateError> {
    let mut command =
        if cfg!(target_os = "macos") && installed.extension().is_some_and(|e| e == "app") {
            // Why: `open -n` hands the bundle to launchd, which gives the new
            // instance a proper session — spawning Contents/MacOS/<bin> directly
            // leaves it parented to a dying process and without one.
            let mut c = std::process::Command::new("/usr/bin/open");
            c.arg("-n").arg(installed);
            c
        } else {
            #[cfg_attr(
                not(target_os = "windows"),
                expect(unused_mut, reason = "`no_window` borrows mutably only on Windows")
            )]
            let mut c = std::process::Command::new(installed);
            #[cfg(target_os = "windows")]
            crate::winproc::no_window(&mut c);
            c
        };
    command
        .spawn()
        .map(|child| {
            tracing::info!(pid = child.id(), path = %installed.display(), "update: relaunched");
        })
        .map_err(UpdateError::Relaunch)
}


pub async fn run_automatic(gateway: &ValidatedUrl, bearer: &str) {
    if !automatic_enabled() {
        tracing::warn!(
            "a newer bridge is required but automatic updates are disabled by policy; \
             update manually",
        );
        return;
    }
    let client = GatewayClient::new(gateway.clone());
    let (status, manifest) = match check(&client, bearer).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!(error = %e, "automatic update: could not read the release manifest");
            return;
        },
    };
    if matches!(status, UpdateStatus::Current { .. }) {
        tracing::warn!(
            local = %crate::brand::brand().version,
            "the gateway reports this bridge as unsupported but offers no newer release",
        );
        return;
    }
    tracing::info!(version = %manifest.version, "automatic update: installing");
    let installed = match apply(&client, bearer, &manifest, &|_| {}).await {
        Ok(path) => path,
        Err(e) => {
            tracing::error!(error = %e, "automatic update: install failed");
            return;
        },
    };
    if let Err(e) = spawn_installed(&installed) {
        tracing::error!(error = %e, "automatic update: relaunch failed; update is staged on disk");
        return;
    }
    tracing::info!(version = %manifest.version, "automatic update: relaunched");
}

#[must_use]
pub fn automatic_enabled() -> bool {
    crate::config::load()
        .update
        .as_ref()
        .and_then(|u| u.automatic)
        .unwrap_or(true)
}
