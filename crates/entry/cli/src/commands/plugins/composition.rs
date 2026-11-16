//! The admitted composition as the `plugins` commands read it.
//!
//! Every compiled-extension view (`list`, `show`, `config`, `capabilities`,
//! `seams`) reads the members the composition admitted against the active
//! profile, in composition order. A member the profile disables is not
//! admitted and so is not listed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::admitted::AdmittedMember;
use systemprompt_extension::contribute::StandingTag;
use systemprompt_extension::{Admitted, BuiltState};
use systemprompt_extension::cli::{CommandError, CommandHost};

use super::types::{CapabilitySummary, ExtensionStanding};

pub(super) fn built(host: &dyn CommandHost) -> Result<&BuiltState, CommandError> {
    host.admitted().map(Admitted::built)
}

pub(super) fn matching<'a>(
    built: &'a BuiltState,
    filter: Option<&'a str>,
) -> impl Iterator<Item = &'a AdmittedMember> + 'a {
    let needle = filter.map(str::to_lowercase);
    built.members().filter(move |member| {
        needle.as_ref().is_none_or(|needle| {
            member.metadata.id.to_lowercase().contains(needle.as_str())
                || member.metadata.name.to_lowercase().contains(needle.as_str())
        })
    })
}

pub(super) fn find<'a>(built: &'a BuiltState, id: &str) -> Option<&'a AdmittedMember> {
    let needle = id.to_lowercase();
    built.members().find(|member| {
        member.metadata.id.to_lowercase() == needle || member.metadata.name.to_lowercase() == needle
    })
}

pub(super) const fn standing(member: &AdmittedMember) -> ExtensionStanding {
    match member.standing {
        StandingTag::Core => ExtensionStanding::Core,
        StandingTag::Install => ExtensionStanding::Install,
    }
}

pub(super) fn capabilities(member: &AdmittedMember) -> CapabilitySummary {
    CapabilitySummary {
        jobs: member.sink.jobs().len(),
        schemas: member.sink.schemas().len(),
        roles: member.sink.roles().len(),
        configs: member.sink.configs().len(),
    }
}
