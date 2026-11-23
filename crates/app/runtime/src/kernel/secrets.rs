//! The secret names an install may declare and read.
//!
//! The kernel builds every member's secret-reference factories (scoped to the
//! member), joins them with its own references — the platform's pinned keys,
//! the profile's licence signing secret and every secret ref of the live
//! services config — and refuses, typed, a member that declared a name any of
//! them references. The accepted declarations are inserted sealed as one
//! [`SecretDeclarations`] over the live secret store; the references are
//! consulted again on every read, so a name a services reload starts to
//! reference is refused from then on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::{BuiltState, Resolved, SecretDeclarations, build_factories};
use systemprompt_manifest::profile::LICENCE_SIGNING_SECRET;
use systemprompt_manifest::services::ServicesView;
use systemprompt_traits::{PINNED_SECRET_KEYS, SecretReferences, SecretStore};

use super::KernelError;

const SECRET_REFERENCES: &str = "secret_references";

struct KernelSecretReferences {
    services: Option<ServicesView>,
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
        PINNED_SECRET_KEYS
            .iter()
            .chain(std::iter::once(&LICENCE_SIGNING_SECRET))
            .any(|pinned| pinned.eq_ignore_ascii_case(name))
            || self
                .services
                .as_ref()
                .is_some_and(|services| services.config().references_secret(name))
            || self
                .contributed
                .iter()
                .any(|references| references.references(name))
    }
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
    let references: Arc<dyn SecretReferences> = Arc::new(KernelSecretReferences {
        services: r.optional::<ServicesView>(),
        contributed,
    });
    let mut declared = HashMap::new();
    for member in built.members() {
        let names = member.sink.declared_secrets();
        if let Some(name) = names
            .iter()
            .find(|name| references.references(name.as_str()))
        {
            return Err(KernelError::ReservedSecret {
                owner: member.owner.clone(),
                name: name.clone(),
            });
        }
        declared.insert(member.owner.clone(), Arc::from(names.to_vec()));
    }
    Ok(SecretDeclarations::new(
        r.optional::<Arc<dyn SecretStore>>(),
        references,
        declared,
    ))
}
