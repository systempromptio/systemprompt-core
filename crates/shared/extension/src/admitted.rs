//! The admission handoff from the composition to the kernel.
//!
//! The facade builds an [`Admitted`] after it has dropped every disabled
//! member's contributions and admitted the rest against the conferral table;
//! the API mount and the runtime read it. [`DeclaredCommands`] is the
//! profile-independent half: every member's CLI commands, admitted against
//! the reserved CLI paths before the profile is loaded, so the CLI can build
//! its command tree first. The CLI dispatches only from it, and decides
//! whether a command's owner is disabled from the profile itself.
//!
//! [`Admitted::withheld`] lists every member the composition withheld with a
//! member it depends on (disabled in the profile, removed from the
//! composition, or itself withheld), with that dependency; the host reports
//! each one at boot and serves none of its contributions.
//!
//! Both are constructed only through a hidden `__from_facade` constructor. The
//! kernel re-checks every privileged entry against the constants in
//! [`reserved`](crate::reserved), so a handoff built any other way gains
//! nothing beyond a shape those constants list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;

use crate::contribute::{ContributedJob, ContributionSink, MemberScope, StandingTag};

/// One admitted member's contributions, minus the routes and commands the
/// handoff carries separately.
#[derive(Debug)]
pub struct AdmittedMember {
    pub owner: ExtensionId,
    pub metadata: crate::metadata::ExtensionMetadata,
    pub standing: StandingTag,
    pub sink: ContributionSink,
    pub secret_grants: Vec<systemprompt_traits::SecretPattern>,
}

/// A member the composition withheld with a member it depends on
/// (`Contributions::depends_on`): none of its contributions is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberWithheld {
    pub owner: ExtensionId,
    pub dependency: ExtensionId,
    pub because: WithheldBecause,
}

/// Why a [`MemberWithheld`]'s dependency is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithheldBecause {
    DependencyDisabled,
    DependencyRemoved,
    DependencyWithheld,
}

impl WithheldBecause {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyDisabled => "dependency_disabled",
            Self::DependencyRemoved => "dependency_removed",
            Self::DependencyWithheld => "dependency_withheld",
        }
    }
}

/// Every admitted member's contributions, in composition order.
#[derive(Debug, Default)]
pub struct BuiltState {
    members: Vec<AdmittedMember>,
}

impl AdmittedMember {
    #[must_use]
    pub fn scope(&self) -> MemberScope {
        MemberScope::admitted(self.owner.clone(), self.standing)
    }
}

impl BuiltState {
    pub fn members(&self) -> impl Iterator<Item = &AdmittedMember> {
        self.members.iter()
    }

    pub fn jobs(&self) -> impl Iterator<Item = (&ExtensionId, &ContributedJob)> {
        self.members.iter().flat_map(|member| {
            member
                .sink
                .jobs()
                .iter()
                .map(move |job| (&member.owner, job))
        })
    }

    #[must_use]
    pub fn contains(&self, owner: &ExtensionId) -> bool {
        self.members.iter().any(|member| &member.owner == owner)
    }

    #[must_use]
    pub fn standing_of(&self, owner: &ExtensionId) -> Option<StandingTag> {
        self.members
            .iter()
            .find(|member| &member.owner == owner)
            .map(|member| member.standing)
    }
}

/// The parts the facade assembles an [`Admitted`] from.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct AdmittedParts {
    pub members: Vec<AdmittedMember>,
    pub withheld: Vec<MemberWithheld>,
    #[cfg(feature = "router")]
    pub routes: Vec<AdmittedRouteSet>,
    #[cfg(feature = "router")]
    pub gateway_routes: Vec<AdmittedGatewayRoute>,
}

#[derive(Debug)]
pub struct Admitted {
    built: BuiltState,
    withheld: Vec<MemberWithheld>,
    #[cfg(feature = "router")]
    routes: Vec<AdmittedRouteSet>,
    #[cfg(feature = "router")]
    gateway_routes: Vec<AdmittedGatewayRoute>,
}

impl Admitted {
    #[doc(hidden)]
    #[must_use]
    pub fn __from_facade(parts: AdmittedParts) -> Self {
        Self {
            built: BuiltState {
                members: parts.members,
            },
            withheld: parts.withheld,
            #[cfg(feature = "router")]
            routes: parts.routes,
            #[cfg(feature = "router")]
            gateway_routes: parts.gateway_routes,
        }
    }

    #[cfg(feature = "router")]
    #[must_use]
    pub fn routes(&self) -> &[AdmittedRouteSet] {
        &self.routes
    }

    #[cfg(feature = "router")]
    #[must_use]
    pub fn gateway_routes(&self) -> &[AdmittedGatewayRoute] {
        &self.gateway_routes
    }

    #[must_use]
    pub const fn built(&self) -> &BuiltState {
        &self.built
    }

    #[must_use]
    pub fn withheld(&self) -> &[MemberWithheld] {
        &self.withheld
    }
}

/// One admitted route set, with the owner the composition stamped and the
/// kernel handles it conferred on the set's base.
#[cfg(feature = "router")]
#[derive(Debug)]
pub struct AdmittedRouteSet {
    pub owner: ExtensionId,
    pub standing: StandingTag,
    pub set: crate::router::RouteSet,
    pub handles: Vec<crate::contribute::HandleId>,
}

/// One admitted gateway route, with its stamped owner.
#[cfg(feature = "router")]
#[derive(Debug)]
pub struct AdmittedGatewayRoute {
    pub owner: ExtensionId,
    pub decl: crate::router::GatewayRouteDecl,
}

/// One admitted CLI command, with the owner the composition stamped.
///
/// The stamp is the member id and the core id its core terms carry (`None`
/// for an install or a core member with no conferral). The CLI registry
/// admits a reserved path only for the entry whose stamped core id owns it.
#[cfg(feature = "cli")]
#[derive(Debug, Clone)]
pub struct AdmittedCommand {
    pub owner: ExtensionId,
    pub core: Option<crate::reserved::CoreId>,
    pub decl: crate::cli::CommandDecl,
}

/// The parts the facade assembles [`DeclaredCommands`] from.
#[doc(hidden)]
#[derive(Debug, Default)]
#[cfg_attr(not(feature = "cli"), derive(Clone, Copy))]
pub struct DeclaredCommandParts {
    #[cfg(feature = "cli")]
    pub commands: Vec<AdmittedCommand>,
}

/// Every member's declared CLI commands, already admitted against the
/// reserved CLI paths and the CLI conferrals. The CLI builds its command tree
/// from it before it loads the profile.
#[derive(Debug, Default)]
#[cfg_attr(not(feature = "cli"), derive(Clone, Copy))]
pub struct DeclaredCommands {
    #[cfg(feature = "cli")]
    commands: Vec<AdmittedCommand>,
}

impl DeclaredCommands {
    #[doc(hidden)]
    #[must_use]
    #[cfg_attr(
        not(feature = "cli"),
        expect(
            clippy::missing_const_for_fn,
            reason = "with the `cli` feature the parts own a Vec, whose drop cannot run in a const fn"
        )
    )]
    pub fn __from_facade(parts: DeclaredCommandParts) -> Self {
        #[cfg(not(feature = "cli"))]
        let DeclaredCommandParts {} = parts;
        Self {
            #[cfg(feature = "cli")]
            commands: parts.commands,
        }
    }

    #[cfg(feature = "cli")]
    #[must_use]
    pub fn commands(&self) -> &[AdmittedCommand] {
        &self.commands
    }
}
