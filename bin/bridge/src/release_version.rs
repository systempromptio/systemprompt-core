//! Release-version ordering shared by the update decision, the staged-update
//! record and the second-launch takeover.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn is_newer(candidate: &str, baseline: &str) -> bool {
    match (
        semver::Version::parse(candidate),
        semver::Version::parse(baseline),
    ) {
        (Ok(candidate), Ok(baseline)) => candidate.cmp_precedence(&baseline).is_gt(),
        _ => false,
    }
}
