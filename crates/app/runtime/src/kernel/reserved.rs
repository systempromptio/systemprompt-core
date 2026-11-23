//! The kernel's own resolution steps over the admitted members: the
//! reservation of the types only the kernel inserts, the refusal of a
//! conferred provide its reserved row does not name, and the gateway guard
//! set it builds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::sync::Arc;

use systemprompt_extension::contribute::ProvideEntry;
use systemprompt_extension::{
    AdmittedOwnerReassignments, AudienceResolvers, BuiltState, GatewayGuards, Resolved, StandingTag,
};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::policy::PolicyCatalog;

use super::KernelError;

const fn kernel_inserted() -> [TypeId; 5] {
    [
        TypeId::of::<Arc<AudienceResolvers>>(),
        TypeId::of::<Arc<AdmittedOwnerReassignments>>(),
        TypeId::of::<Arc<SubjectProviderSet>>(),
        TypeId::of::<Arc<PolicyCatalog>>(),
        TypeId::of::<Arc<GatewayGuards>>(),
    ]
}

pub(super) fn refuse_kernel_types(built: &BuiltState) -> Result<(), KernelError> {
    let reserved = kernel_inserted();
    for member in built.members() {
        let provided = member
            .sink
            .provides()
            .iter()
            .map(|entry| (entry.type_id(), entry.type_name()));
        let slot_built = member
            .sink
            .slots()
            .iter()
            .map(|record| (record.built().type_id(), record.built().type_name()));
        if let Some((_, ty)) = provided
            .chain(slot_built)
            .find(|(id, _)| reserved.contains(id))
        {
            return Err(KernelError::ReservedType {
                owner: member.owner.clone(),
                ty,
            });
        }
    }
    Ok(())
}

pub(super) fn refuse_unlisted_conferrals(built: &BuiltState) -> Result<(), KernelError> {
    for member in built.members() {
        for row in member
            .sink
            .provides()
            .iter()
            .filter_map(ProvideEntry::conferral)
        {
            let listed = row.entry().provider == member.owner.as_str();
            if !listed || member.standing != StandingTag::Core {
                return Err(KernelError::UnlistedConferral {
                    owner: member.owner.clone(),
                    row: row.name(),
                });
            }
        }
    }
    Ok(())
}

pub(super) fn gateway_guards(
    built: &BuiltState,
    r: &Resolved<'_>,
) -> Result<GatewayGuards, KernelError> {
    let mut installed = Vec::new();
    for member in built.members() {
        for factory in member.sink.gateway_guards() {
            let guard = factory
                .build(r)
                .map_err(|source| KernelError::GatewayGuard {
                    owner: member.owner.clone(),
                    source,
                })?;
            installed.push((member.owner.clone(), guard));
        }
    }
    Ok(GatewayGuards::new(installed))
}
