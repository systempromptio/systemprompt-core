//! Doctor checks for `OpenCode`.
//!
//! Which tier owns the provider, whether the installed client reads the V2
//! config the bridge writes, and whether it kept every policy and the
//! provider it was given. The client is probed only when the last sync
//! enabled an `OpenCode` packaging: an `OpenCode` the bridge does not drive is
//! not this device's fault.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Check;
use systemprompt_models::bridge::host::HostKind;

use crate::integration::opencode::{
    ClientVersion, RoundTrip, probe_client_version, probe_round_trip,
};

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
pub fn check_round_trip(round_trip: &RoundTrip) -> Option<Check> {
    const NAME: &str = "opencode config round-trip";
    match round_trip {
        RoundTrip::NotInstalled | RoundTrip::NothingWritten => None,
        RoundTrip::Kept => Some(Check::ok(
            NAME,
            "the loaded config keeps the provider and every policy, in order",
        )),
        RoundTrip::Unreadable(reason) => Some(Check::warn(
            NAME,
            format!("`opencode debug config` could not be read: {reason}"),
        )),
        RoundTrip::Lost(lost) => Some(Check::fail(
            NAME,
            format!(
                "{}; OpenCode drops a malformed entry without a message. If an \
                 `opencode serve` service is running it may hold an older \
                 config: quit OpenCode and re-run doctor",
                lost.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )),
    }
}

#[must_use]
pub fn opencode_enabled(enabled: &[HostKind]) -> bool {
    enabled
        .iter()
        .any(|host| matches!(host, HostKind::OpenCode | HostKind::OpenCodeCli))
}

#[must_use]
pub fn checks(enabled: Option<&[HostKind]>) -> Vec<Check> {
    let mut out: Vec<Check> = check_provider_tier().into_iter().collect();
    if enabled.is_some_and(opencode_enabled) {
        let version = probe_client_version();
        out.extend(check_client_version(&version));
        if version.is_supported() {
            out.extend(check_round_trip(&probe_round_trip()));
        }
    }
    out
}
