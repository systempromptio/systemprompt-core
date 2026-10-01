//! Which hosts a signed manifest enables and which hosts each of its skills
//! targets. The manifest carries host ids as strings, so an id this build
//! does not know is reported rather than matched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::{SignedManifest, SkillEntry};

#[must_use]
pub fn enables_host(manifest: &SignedManifest, host: HostKind) -> bool {
    manifest.enabled_hosts.iter().any(|h| h == host.as_str())
}

#[must_use]
pub fn skill_targets_host(skill: &SkillEntry, host: HostKind) -> bool {
    skill.hosts.is_empty() || skill.hosts.iter().any(|h| h == host.as_str())
}

pub fn unknown_skill_hosts(manifest: &SignedManifest) -> impl Iterator<Item = (&str, &str)> {
    manifest.skills.iter().flat_map(|skill| {
        skill
            .hosts
            .iter()
            .filter(|h| h.parse::<HostKind>().is_err())
            .map(move |h| (skill.id.as_str(), h.as_str()))
    })
}
