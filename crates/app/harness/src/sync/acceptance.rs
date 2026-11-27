//! The rules a verified manifest passes before it is used.
//!
//! The manifest is the account's user's, it carries a `manifest_version`
//! strictly newer than the last one applied for the account, and its
//! `not_before` lies within the clock-skew window (only a date too far ahead
//! once a version was applied, either way before that). A 304 never comes here:
//! its stored manifest passed when it first came.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::version::{dated_beyond_skew, is_newer};
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::manifest_version::ManifestVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the manifest names another user than the account's")]
    User,
    #[error("the manifest carries no manifest version")]
    Version,
    #[error("the manifest carries no not_before, or one outside the clock-skew window")]
    Skew,
    #[error("the manifest is not newer than the last one applied")]
    Replay,
}

pub fn accept(
    manifest: &Manifest,
    expected_user: Option<&UserId>,
    last_applied: Option<&ManifestVersion>,
    now: DateTime<Utc>,
) -> Result<ManifestVersion, Refusal> {
    if expected_user.is_some_and(|user| user != &manifest.user_id) {
        return Err(Refusal::User);
    }
    let version = manifest.manifest_version.clone();
    let not_before = manifest.not_before;
    if dated_beyond_skew(not_before, now, last_applied.is_some()) {
        return Err(Refusal::Skew);
    }
    if !is_newer(&version, last_applied) {
        return Err(Refusal::Replay);
    }
    Ok(version)
}
