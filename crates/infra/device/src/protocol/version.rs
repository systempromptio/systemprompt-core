//! The manifest version and the acceptance rules that only need it and a clock.
//!
//! A `ManifestVersion` is `<RFC 3339 timestamp>-<hex suffix>` (at least 8 hex
//! digits). Versions order by timestamp, then by suffix, so a manifest is
//! accepted only when it is strictly newer than the last one accepted. The type
//! is `systemprompt-models`' own, so the harness orders versions exactly as the
//! instance mints them. An envelope dated more than the skew window ahead is
//! refused always; one dated that far behind only before any acceptance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_models::bridge::manifest_version::ManifestVersion;

pub const SKEW_WINDOW_MINUTES: i64 = 5;

#[must_use]
pub fn is_newer(candidate: &ManifestVersion, accepted: Option<&ManifestVersion>) -> bool {
    !already_covered(candidate, accepted)
}

fn already_covered(candidate: &ManifestVersion, accepted: Option<&ManifestVersion>) -> bool {
    accepted.is_some_and(|accepted| candidate <= accepted)
}

#[must_use]
pub fn dated_beyond_skew(
    not_before: DateTime<Utc>,
    now: DateTime<Utc>,
    accepted_before: bool,
) -> bool {
    let window = Duration::minutes(SKEW_WINDOW_MINUTES);
    let ahead = not_before.signed_duration_since(now);
    if accepted_before {
        return ahead > window;
    }
    ahead > window || ahead < -window
}
