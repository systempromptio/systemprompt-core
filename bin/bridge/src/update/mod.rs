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

pub use download::{DownloadProgress, download_verified};
pub use error::UpdateError;

use crate::gateway::GatewayClient;
use crate::gateway::types::ReleaseManifest;

/// The artifact key this build asks the gateway for. Matches the release
/// workflow's build matrix; `None` where we publish nothing.
///
/// Written as one match over the target constants rather than `cfg` blocks so
/// every platform compiles the same shape — a `cfg`-per-arm version reads as
/// "always returns `Some`" on macOS and Windows, which is a lint and a lie.
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
    /// Nothing newer is published. Also covers a local build *ahead* of the
    /// gateway, which is normal on a development machine.
    Current { version: String },
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
    /// No check has completed yet. The button shows nothing.
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
    /// Swapped in; the GUI offers a restart rather than forcing one.
    Ready {
        version: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateUiState {
    /// The version this state refers to, so a progress update can be matched to
    /// the release that started it.
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

/// Compares the gateway's newest published build against this one.
///
/// Both versions must parse as semver; a malformed version on either side is an
/// error rather than a silent "no update", so a broken manifest is visible
/// instead of quietly pinning every client forever.
pub async fn check(
    client: &GatewayClient,
    bearer: &str,
) -> Result<(UpdateStatus, ReleaseManifest), UpdateError> {
    let platform = platform_slug().ok_or(UpdateError::UnsupportedPlatform)?;
    let manifest = client.fetch_latest_release(bearer, platform).await?;
    let status = compare(crate::brand::brand().version, &manifest)?;
    Ok((status, manifest))
}

fn compare(local: &str, manifest: &ReleaseManifest) -> Result<UpdateStatus, UpdateError> {
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

/// Downloads, verifies, and swaps in the build described by `manifest`.
///
/// Returns the path that now holds the new build. Does **not** relaunch — the
/// caller decides, because the GUI wants a restart and the CLI does not.
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

/// Housekeeping for swap leftovers, called once at startup. Only Windows
/// leaves any: it renames the running exe aside because it cannot overwrite it.
pub fn sweep_leftovers() {
    install::sweep_leftovers();
}

/// Where the running install lives — the `.app` bundle on macOS, the executable
/// elsewhere. This is what [`spawn_installed`] relaunches after a swap.
pub fn installed_path() -> Result<std::path::PathBuf, UpdateError> {
    install::installed_path()
}

/// Starts the freshly-installed build as a detached process.
///
/// The caller exits immediately afterwards; on Windows in particular the old
/// process must release its image lock before the leftover can be swept.
pub fn spawn_installed(installed: &std::path::Path) -> Result<(), UpdateError> {
    let mut command = if cfg!(target_os = "macos") && installed.extension().is_some_and(|e| e == "app")
    {
        // Why: `open -n` hands the bundle to launchd, which gives the new
        // instance a proper session — spawning Contents/MacOS/<bin> directly
        // leaves it parented to a dying process and without one.
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg("-n").arg(installed);
        c
    } else {
        std::process::Command::new(installed)
    };
    command
        .spawn()
        .map(|child| {
            tracing::info!(pid = child.id(), path = %installed.display(), "update: relaunched");
        })
        .map_err(UpdateError::Relaunch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> ReleaseManifest {
        ReleaseManifest {
            version: version.to_owned(),
            sha256: "0".repeat(64),
            size: 1,
            notes_url: None,
        }
    }

    #[test]
    fn newer_remote_is_available() {
        assert!(matches!(
            compare("0.1.6", &manifest("0.1.7")),
            Ok(UpdateStatus::Available { ref version, .. }) if version == "0.1.7"
        ));
    }

    #[test]
    fn equal_version_is_current() {
        assert!(matches!(
            compare("0.1.6", &manifest("0.1.6")),
            Ok(UpdateStatus::Current { .. })
        ));
    }

    /// Normal on a development machine, and must not offer a downgrade.
    #[test]
    fn local_ahead_of_gateway_is_current() {
        assert!(matches!(
            compare("0.2.0", &manifest("0.1.6")),
            Ok(UpdateStatus::Current { .. })
        ));
    }

    /// The case a lexical string compare gets wrong.
    #[test]
    fn patch_ten_is_newer_than_patch_nine() {
        assert!(matches!(
            compare("0.1.9", &manifest("0.1.10")),
            Ok(UpdateStatus::Available { .. })
        ));
    }

    #[test]
    fn prerelease_is_older_than_its_release() {
        assert!(matches!(
            compare("0.1.7-rc.1", &manifest("0.1.7")),
            Ok(UpdateStatus::Available { .. })
        ));
    }

    #[test]
    fn malformed_remote_version_is_an_error() {
        assert!(matches!(
            compare("0.1.6", &manifest("latest")),
            Err(UpdateError::BadRemoteVersion { .. })
        ));
    }
}
