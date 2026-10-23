//! Doctor checks for `OpenCode`: which tier owns the provider, and whether
//! the installed client reads the V2 config the bridge writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Check;
use crate::integration::opencode::{ClientVersion, probe_client_version};

#[must_use]
pub fn check_provider_tier() -> Option<Check> {
    let source = crate::integration::opencode::managed_provider_source()?;
    Some(Check::ok(
        "opencode provider",
        format!(
            "managed by MDM ({}); the bridge writes only its token file and content",
            source.display()
        ),
    ))
}

#[must_use]
pub fn check_client_version(version: &ClientVersion) -> Option<Check> {
    let min = systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;
    match version {
        ClientVersion::NotInstalled => None,
        ClientVersion::Found(found) if version.is_supported() => Some(Check::ok(
            "opencode version",
            format!("{found} (V2 config, >= {min})"),
        )),
        ClientVersion::Found(found) => Some(Check::fail(
            "opencode version",
            format!("{found} is older than {min}; upgrade OpenCode to read the bridge's config"),
        )),
        ClientVersion::Unreadable(reason) => Some(Check::warn(
            "opencode version",
            format!("`opencode --version` could not be read: {reason}"),
        )),
    }
}

#[must_use]
pub fn checks() -> Vec<Check> {
    let mut out: Vec<Check> = check_provider_tier().into_iter().collect();
    out.extend(check_client_version(&probe_client_version()));
    out
}
