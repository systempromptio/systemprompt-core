//! The secret names a member may declare and read.
//!
//! A declaration admits nothing on its own: the operator grants each member
//! its secret names and prefixes in the profile (`extensions.secrets`), and
//! the kernel refuses, typed, a member whose declaration no grant covers.
//!
//! The kernel also builds every member's secret-reference factories (scoped
//! to the member), joins them with its own references — the core secret
//! names, every secret field of the profile and of the live services config
//! — and refuses a member that declared a name any of them references. A
//! grant does not lift that refusal. The accepted declarations and grants are
//! inserted sealed as one [`SecretDeclarations`] over the live secret store;
//! the grant and the references are consulted again on every read, so a name
//! a services reload starts to reference is refused from then on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::admitted::AdmittedMember;
use systemprompt_extension::{
    BuiltState, MemberSecrets, Resolved, SecretDeclarations, build_factories,
};
use systemprompt_manifest::profile::ProfileSecretReferences;
use systemprompt_manifest::services::ServicesView;
use systemprompt_traits::{SecretPattern, SecretReferences, SecretStore, is_core_secret_name};

use super::KernelError;

const SECRET_REFERENCES: &str = "secret_references";

struct KernelSecretReferences {
    services: Option<ServicesView>,
    profile: Option<ProfileSecretReferences>,
    contributed: Vec<Arc<dyn SecretReferences>>,
}

impl std::fmt::Debug for KernelSecretReferences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelSecretReferences")
            .field("contributed", &self.contributed)
            .finish_non_exhaustive()
    }
}

impl SecretReferences for KernelSecretReferences {
    fn references(&self, name: &str) -> bool {
        is_core_secret_name(name)
            || self
                .services
                .as_ref()
                .is_some_and(|services| services.config().references_secret(name))
            || self
                .profile
                .as_ref()
                .is_some_and(|profile| profile.references(name))
            || self
                .contributed
                .iter()
                .any(|references| references.references(name))
    }
}

fn admitted_secrets(
    member: &AdmittedMember,
    references: &dyn SecretReferences,
) -> Result<MemberSecrets, KernelError> {
    let declared = member.sink.declared_secrets();
    for pattern in declared {
        if !member
            .secret_grants
            .iter()
            .any(|grant| grant.covers_pattern(pattern))
        {
            return Err(KernelError::UngrantedSecret {
                owner: member.owner.clone(),
                pattern: pattern.clone(),
            });
        }
        if let SecretPattern::Exact(name) = pattern
            && references.references(name.as_str())
        {
            return Err(KernelError::ReservedSecret {
                owner: member.owner.clone(),
                name: pattern.clone(),
            });
        }
    }
    Ok(MemberSecrets {
        declared: Arc::from(declared.to_vec()),
        granted: Arc::from(member.secret_grants.clone()),
    })
}

pub(super) fn declarations(
    built: &BuiltState,
    r: &Resolved<'_>,
) -> Result<SecretDeclarations, KernelError> {
    let mut contributed = Vec::new();
    for member in built.members() {
        contributed.extend(build_factories(
            SECRET_REFERENCES,
            &member.scope(),
            member.sink.secret_references(),
            r,
        )?);
    }
    let references = KernelSecretReferences {
        services: r.optional::<ServicesView>(),
        profile: r.optional::<ProfileSecretReferences>(),
        contributed,
    };
    let mut declared = HashMap::new();
    for member in built.members() {
        declared.insert(member.owner.clone(), admitted_secrets(member, &references)?);
    }
    Ok(SecretDeclarations::new(
        r.optional::<Arc<dyn SecretStore>>(),
        Arc::new(references),
        declared,
    ))
}
