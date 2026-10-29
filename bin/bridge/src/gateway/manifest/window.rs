//! The per-instance release window a verified manifest sets, judged against
//! this bridge's release (brand) version by strict semver precedence.
//!
//! The wire floor (`min_bridge_version` against [`COMPAT_VERSION`]) is checked
//! separately in [`super::decode_payload`]; this window is the instance's own
//! choice of releases and is applied only where a sync acts on the manifest.
//!
//! [`COMPAT_VERSION`]: crate::brand::COMPAT_VERSION
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::instance::BridgeVersionWindow;

use super::{ManifestError, SignedManifest};

/// Where this release sits in a window the manifest did not refuse outright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowVerdict {
    Admitted,
    AboveTolerated { max: semver::Version },
}

pub fn check_release_window(
    manifest: &SignedManifest,
    local: &str,
) -> Result<WindowVerdict, ManifestError> {
    let Some(window) = manifest.bridge_versions.as_ref() else {
        return Ok(WindowVerdict::Admitted);
    };
    judge(window, local)
}

pub fn judge(window: &BridgeVersionWindow, local: &str) -> Result<WindowVerdict, ManifestError> {
    let version = semver::Version::parse(local).map_err(|source| ManifestError::LocalVersion {
        local: local.to_owned(),
        source,
    })?;
    if let Some(min) = window.min.as_ref()
        && version.cmp_precedence(min).is_lt()
    {
        return Err(ManifestError::BridgeBelowWindow {
            local: local.to_owned(),
            min: min.to_string(),
        });
    }
    match window.max.as_ref() {
        Some(max) if version.cmp_precedence(max).is_gt() && window.enforce_max => {
            Err(ManifestError::BridgeAboveWindow {
                local: local.to_owned(),
                max: max.to_string(),
            })
        },
        Some(max) if version.cmp_precedence(max).is_gt() => {
            Ok(WindowVerdict::AboveTolerated { max: max.clone() })
        },
        _ => Ok(WindowVerdict::Admitted),
    }
}
