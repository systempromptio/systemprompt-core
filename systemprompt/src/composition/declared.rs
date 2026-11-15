//! Every member's declarations before the profile is applied, and their
//! admission against it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::collections::{HashMap, HashSet};

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts, DeclaredCommandParts};
use systemprompt_extension::contribute::{ContributionSink, Contributions};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::{
    Admitted, ContributionError, DeclaredCommands, Extension, ExtensionConfigError, SlotError,
    StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_manifest::Profile;
use systemprompt_traits::authz::SubjectDimension;

#[cfg(feature = "api")]
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
#[cfg(feature = "api")]
use systemprompt_extension::router::{Allowance, BasePath, BaseRef, RouteSet};
#[cfg(feature = "cli")]
use systemprompt_extension::admitted::AdmittedCommand;
#[cfg(feature = "cli")]
use systemprompt_extension::cli::KernelGrant;



use super::admit::{check_providers, disabled_members};
#[cfg(feature = "api")]
use super::routes::{admit_gateway_route, admit_route_set, check_gateway_overlaps, check_route_conflicts};
use super::{CompositionError, SecretsKind, Standing};

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
    #[cfg(feature = "cli")]
    pub(super) commands: Vec<AdmittedCommand>,
    #[cfg(feature = "config")]
    pub(super) secrets: Vec<(SecretsKind, systemprompt_config::SecretsSourceFactory)>,
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
    pub fn secrets_sources(&self) -> &[(SecretsKind, systemprompt_config::SecretsSourceFactory)] {
        &self.secrets
    }

    #[must_use]
    pub fn commands(&self) -> DeclaredCommands {
        DeclaredCommands::__from_facade(DeclaredCommandParts {
            #[cfg(feature = "cli")]
            commands: self.commands.clone(),
        })
    }

    pub fn admit(self, profile: &Profile) -> Result<Admitted, CompositionError> {
        let disabled = disabled_members(&self.members, profile)?;
        let (enabled, dropped): (Vec<DeclaredMember>, Vec<DeclaredMember>) = self
            .members
            .into_iter()
            .partition(|member| !disabled.contains(&member.id));
        check_providers(&enabled, &dropped)?;
        drop(dropped);
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
            #[cfg(feature = "cli")]
            drop(member.sink.take_commands());
            members.push(AdmittedMember {
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
            #[cfg(feature = "api")]
            routes,
            #[cfg(feature = "api")]
            gateway_routes,
        }))
    }
}
