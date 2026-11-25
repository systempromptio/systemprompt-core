//! Every member's declarations before the profile is applied, and their
//! admission against it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts, DeclaredCommandParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{AbsentDependency, Admitted, DeclaredCommands, MemberWithheld};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::ExtensionsConfig;

#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;

use super::admit::{disabled_members, granted_members, withhold_dependants};
use super::providers::check_providers;
#[cfg(feature = "api")]
use super::routes::{
    admit_gateway_route, admit_route_set, check_gateway_overlaps, check_route_conflicts,
};
use super::{CompositionError, Standing};

pub(super) struct DeclaredMember {
    pub(super) metadata: systemprompt_extension::ExtensionMetadata,
    pub(super) id: ExtensionId,
    pub(super) standing: Standing,
    pub(super) type_id: TypeId,
    pub(super) sink: ContributionSink,
}

/// Every member's declarations, before the profile is applied.
pub struct Declared {
    pub(super) members: Vec<DeclaredMember>,
    pub(super) withheld: Vec<MemberWithheld>,
    #[cfg(feature = "cli")]
    pub(super) commands: Vec<AdmittedCommand>,
    #[cfg(feature = "config")]
    pub(super) secrets: systemprompt_config::SecretsSources,
}

impl std::fmt::Debug for Declared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Declared")
            .field(
                "members",
                &self.members.iter().map(|m| &m.id).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Declared {
    #[cfg(feature = "config")]
    #[must_use]
    pub fn secrets_sources(&self) -> systemprompt_config::SecretsSources {
        self.secrets.clone()
    }

    #[must_use]
    pub fn commands(&self) -> DeclaredCommands {
        DeclaredCommands::__from_facade(DeclaredCommandParts {
            #[cfg(feature = "cli")]
            commands: self.commands.clone(),
        })
    }

    pub fn admit(self, extensions: &ExtensionsConfig) -> Result<Admitted, CompositionError> {
        self.admit_with(extensions)
    }

    pub fn admit_unprofiled(self) -> Result<Admitted, CompositionError> {
        self.admit_with(&ExtensionsConfig::default())
    }

    fn admit_with(self, extensions: &ExtensionsConfig) -> Result<Admitted, CompositionError> {
        let disabled = disabled_members(&self.members, &extensions.disabled)?;
        granted_members(&self.members, extensions)?;
        let (mut enabled, dropped): (Vec<DeclaredMember>, Vec<DeclaredMember>) = self
            .members
            .into_iter()
            .partition(|member| !disabled.contains(&member.id));
        let absent = dropped
            .iter()
            .map(|member| (member.type_id, member.id.clone()))
            .collect();
        let mut withheld = self.withheld;
        let (cascaded, held_back) =
            withhold_dependants(&mut enabled, absent, AbsentDependency::Disabled)?;
        let carried: Vec<_> = held_back
            .iter()
            .zip(&cascaded)
            .map(|(member, record)| (member, &record.dependency))
            .collect();
        check_providers(&enabled, &dropped, &carried)?;
        drop(carried);
        drop(dropped);
        drop(held_back);
        withheld.extend(cascaded);
        let mut members = Vec::with_capacity(enabled.len());
        #[cfg(feature = "api")]
        let mut routes = Vec::new();
        #[cfg(feature = "api")]
        let mut gateway_routes = Vec::new();
        for mut member in enabled {
            #[cfg(feature = "api")]
            {
                let (sets, gateways) = member.sink.take_routes();
                for set in sets {
                    routes.push(admit_route_set(&member, set)?);
                }
                for decl in gateways {
                    gateway_routes.push(admit_gateway_route(&member, decl)?);
                }
            }
            members.push(AdmittedMember {
                secret_grants: extensions.secret_grants(&member.id).to_vec(),
                owner: member.id,
                metadata: member.metadata,
                standing: member.standing.tag(),
                sink: member.sink,
            });
        }
        #[cfg(feature = "api")]
        {
            check_route_conflicts(&routes)?;
            check_gateway_overlaps(&gateway_routes)?;
        }
        Ok(Admitted::__from_facade(AdmittedParts {
            members,
            withheld,
            #[cfg(feature = "api")]
            routes,
            #[cfg(feature = "api")]
            gateway_routes,
        }))
    }
}
