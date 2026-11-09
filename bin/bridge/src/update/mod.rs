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

pub mod automatic;
pub mod completion;
mod download;
mod error;
mod install;
mod policy;
pub mod staged;
pub mod swap;
mod ui_state;

pub use download::{DownloadProgress, download_verified};
pub use error::UpdateError;
pub use policy::{AutoUpdateDecision, auto_update_policy, automatic_enabled};
pub use staged::StagedUpdate;
pub use ui_state::UpdateUiState;

use crate::release_version::is_newer;

use crate::gateway::GatewayClient;
use crate::gateway::types::ReleaseManifest;
use crate::ids::BearerToken;

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
    Staged {
        version: String,
    },
}

impl UpdateStatus {
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(*self, Self::Available { .. })
    }
}

pub async fn check(
    client: &GatewayClient,
    bearer: &BearerToken,
) -> Result<(UpdateStatus, ReleaseManifest), UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let manifest = client.fetch_latest_release(bearer, platform).await?;
    let staged = staged::pending_for_running();
    let status = decide(
        crate::brand::brand().version,
        staged.as_ref().map(|s| s.version.as_str()),
        &manifest,
        &combined_window()?,
    )?;
    Ok((status, manifest))
}

pub async fn check_now(
    client: &GatewayClient,
    bearer: &BearerToken,
) -> Result<(UpdateStatus, ReleaseManifest), UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let manifest = client.fetch_latest_release_fresh(bearer, platform).await?;
    let staged = staged::pending_for_running();
    let status = decide(
        crate::brand::brand().version,
        staged.as_ref().map(|s| s.version.as_str()),
        &manifest,
        &combined_window()?,
    )?;
    Ok((status, manifest))
}

fn combined_window() -> Result<crate::instance::window::CombinedWindow, UpdateError> {
    crate::instance::window::combined().map_err(|e| UpdateError::Window(Box::new(e)))
}

pub fn decide(
    local: &str,
    staged: Option<&str>,
    manifest: &ReleaseManifest,
    combined: &crate::instance::window::CombinedWindow,
) -> Result<UpdateStatus, UpdateError> {
    let remote = parse_remote(manifest)?;
    let staged = staged.filter(|staged| is_newer(staged, local));
    if staged == Some(manifest.version.as_str()) {
        crate::instance::window::admit_candidate(combined, &remote)?;
        return Ok(UpdateStatus::Staged {
            version: manifest.version.clone(),
        });
    }
    let local_version = parse_local(local)?;
    let ordering = remote.cmp_precedence(&local_version);
    if ordering.is_eq() && staged.is_some() {
        crate::instance::window::admit_candidate(combined, &remote)?;
        return Ok(UpdateStatus::Available {
            version: manifest.version.clone(),
            notes_url: manifest.notes_url.clone(),
        });
    }
    if ordering.is_eq() {
        return Ok(UpdateStatus::Current {
            version: local.to_owned(),
        });
    }
    let local_admitted = crate::instance::window::admit_candidate(combined, &local_version).is_ok();
    if ordering.is_lt() && local_admitted {
        return Ok(UpdateStatus::Current {
            version: local.to_owned(),
        });
    }
    crate::instance::window::admit_candidate(combined, &remote)?;
    Ok(UpdateStatus::Available {
        version: manifest.version.clone(),
        notes_url: manifest.notes_url.clone(),
    })
}

fn parse_remote(manifest: &ReleaseManifest) -> Result<semver::Version, UpdateError> {
    semver::Version::parse(&manifest.version).map_err(|source| UpdateError::BadRemoteVersion {
        version: manifest.version.clone(),
        source,
    })
}

fn parse_local(local: &str) -> Result<semver::Version, UpdateError> {
    semver::Version::parse(local).map_err(|source| UpdateError::BadLocalVersion {
        version: local.to_owned(),
        source,
    })
}

pub async fn apply(
    client: &GatewayClient,
    bearer: &BearerToken,
    manifest: &ReleaseManifest,
    on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<std::path::PathBuf, UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let marker = staged::marker_path().ok_or(UpdateError::NoUpdateDir)?;
    if staged::pending(&marker, crate::brand::brand().version)
        .is_some_and(|staged| staged.version == manifest.version)
    {
        tracing::info!(
            version = %manifest.version,
            "update: this release is already installed and waits for a restart"
        );
        return installed_path();
    }
    let target = install::preflight()?;
    let artifact = download_verified(client, bearer, platform, manifest, on_progress).await?;
    let installed = install::apply(&artifact, target)?;
    if let Err(e) = std::fs::remove_file(&artifact) {
        tracing::debug!(error = %e, path = %artifact.display(), "update: staging cleanup failed");
    }
    install::program_file(&installed)
        .and_then(|program| staged::record_installed(&marker, &manifest.version, &program))
        .map_err(|source| UpdateError::InstalledUnrecorded {
            version: manifest.version.clone(),
            installed: installed.clone(),
            source: Box::new(source),
        })?;
    Ok(installed)
}

pub fn sweep_leftovers() {
    install::sweep_leftovers();
}

pub fn installed_path() -> Result<std::path::PathBuf, UpdateError> {
    install::installed_path()
}

// Why: the successor holds off acquiring the single-instance lock and binding
// the proxy port until this process is gone. Without that wait it loses the
// race for both: it focuses a dying window instead of starting, and it binds a
// fallback port that every host profile written for the default rejects.
pub fn spawn_successor(installed: &std::path::Path) -> Result<(), UpdateError> {
    record_handoff();
    let mut command =
        if cfg!(target_os = "macos") && installed.extension().is_some_and(|e| e == "app") {
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

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn record_handoff() {
    crate::single_instance::record_handoff();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const fn record_handoff() {}
