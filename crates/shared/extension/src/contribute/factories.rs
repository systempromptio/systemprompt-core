//! The deferred constructor of a contributed value ([`Factory`]), built only
//! with a member-scoped resolution; the standing-scoped build of one member's
//! factories ([`build_factories`]); the closed list of factory kinds a sink
//! stores ([`FactoryKind`]); and the boxed factory and shared value types a
//! member's sink stores.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::DoctorCheck;
use systemprompt_provider_contracts::governance::GovernancePolicy;
use systemprompt_traits::authz::SubjectAttributeProvider;
use systemprompt_traits::{RunnerService, SecretReferences};

use super::error::{FactoryError, SlotError};
use super::member::{MemberResolved, MemberScope};
use super::resolved::Resolved;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::gateway_guard::GatewayRequestGuard;
use crate::health::{DiagnosticProbe, ReadinessCheck};
use crate::notifications::NotificationAudienceResolver;

pub type AssetFn = Arc<dyn Fn(&dyn AssetPaths) -> Vec<AssetDefinition> + Send + Sync>;
/// A member's startup configuration validator, built over the services
/// directory the host resolved.
pub type DomainConfigFn =
    fn(&std::path::Path) -> Box<dyn systemprompt_manifest::validators::DomainConfig>;
/// The users member's pre-kernel admin-bootstrap constructor: an open
/// database handle in, the
/// [`DynAdminBootstrap`](systemprompt_traits::DynAdminBootstrap) view out. The
/// CLI host calls it only for a core-standing `users` member.
pub type AdminBootstrapFn =
    fn(
        Arc<dyn systemprompt_traits::DatabaseHandle>,
    )
        -> Result<systemprompt_traits::DynAdminBootstrap, systemprompt_traits::AdminBootstrapError>;

pub type RunnerServiceFactory = Box<dyn Factory<Arc<dyn RunnerService>>>;
pub type ReadinessFactory = Box<dyn Factory<Arc<dyn ReadinessCheck>>>;
pub type DiagnosticFactory = Box<dyn Factory<Arc<dyn DiagnosticProbe>>>;
pub type GatewayGuardFactory = Box<dyn Factory<Arc<dyn GatewayRequestGuard>>>;
pub type SubjectProviderFactory = Box<dyn Factory<Arc<dyn SubjectAttributeProvider>>>;
pub type PolicyFactory = Box<dyn Factory<Arc<dyn GovernancePolicy>>>;
pub type AudienceResolverFactory = Box<dyn Factory<Arc<dyn NotificationAudienceResolver>>>;
pub type SecretReferenceFactory = Box<dyn Factory<Arc<dyn SecretReferences>>>;
pub type DoctorCheckRef = Arc<dyn DoctorCheck>;
#[cfg(feature = "sqlx")]
pub(crate) type DoctorProbeSourceRef = Arc<dyn systemprompt_provider_contracts::DoctorProbeSource>;

/// Every factory kind a member's sink stores.
///
/// Each kind is built only through a [`MemberResolved`];
/// `ContributionSink::factory_count` maps a kind onto its sink field, and
/// that sink walk names every field, so a new factory field must be
/// classified here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactoryKind {
    RunnerService,
    Readiness,
    Diagnostic,
    GatewayGuard,
    SubjectProvider,
    Policy,
    AudienceResolver,
    SecretReferences,
}

impl FactoryKind {
    pub const ALL: &'static [Self] = &[
        Self::RunnerService,
        Self::Readiness,
        Self::Diagnostic,
        Self::GatewayGuard,
        Self::SubjectProvider,
        Self::Policy,
        Self::AudienceResolver,
        Self::SecretReferences,
    ];
}

/// A deferred constructor of one contributed service, run after admission.
/// Built only with a [`MemberResolved`], so it runs scoped to its member.
pub trait Factory<T>: Send + Sync + 'static {
    fn build(&self, r: &MemberResolved<'_>) -> Result<T, FactoryError>;
}

impl<T, F> Factory<T> for F
where
    F: Fn(&Resolved<'_>) -> Result<T, FactoryError> + Send + Sync + 'static,
{
    fn build(&self, r: &MemberResolved<'_>) -> Result<T, FactoryError> {
        self(r)
    }
}

pub fn build_present_factories<T: 'static>(
    slot: &'static str,
    scope: &MemberScope,
    factories: &[Box<dyn Factory<Option<T>>>],
    r: &Resolved<'_>,
) -> Result<Vec<T>, SlotError> {
    let built = build_factories(slot, scope, factories, r)?;
    let declined = built.iter().filter(|value| value.is_none()).count();
    if declined > 0 {
        tracing::info!(
            slot,
            owner = %scope.owner(),
            declined,
            "slot factories declined; the slot holds nothing from them"
        );
    }
    Ok(built.into_iter().flatten().collect())
}

pub fn build_factories<T: 'static>(
    slot: &'static str,
    scope: &MemberScope,
    factories: &[Box<dyn Factory<T>>],
    r: &Resolved<'_>,
) -> Result<Vec<T>, SlotError> {
    let r = MemberResolved::within(r, scope);
    factories
        .iter()
        .map(|factory| {
            factory
                .build(&r)
                .map_err(|error| error.into_slot_error(slot, scope.owner()))
        })
        .collect()
}
