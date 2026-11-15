//! One member's stored contributions.
//!
//! The composition gives every member its own [`ContributionSink`]; the
//! member's [`Contributions`](super::Contributions) only appends to it, so
//! every entry is owned by the member whose sink holds it and no stamping by
//! string is needed. Nothing in a sink runs while it is filled: provide fns,
//! factories, slot builds and asset closures are stored and run only after
//! admission has dropped every disabled member's sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{ConfigPlane, Dependencies, DuplicateDependency, Job};
use systemprompt_traits::{JobRunScope, RunnerService};

use super::config::ConfigDecl;
use super::error::{ContributionError, ProvideError};
use super::resolved::{Factory, Resolved};
use super::slot::SlotRecord;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::gateway_guard::GatewayRequestGuard;
use crate::health::{DiagnosticProbe, MetricDescriptor, ReadinessCheck};
use crate::metadata::{ExtensionRole, SchemaDefinition};
use crate::migration::Migration;
use crate::protection::SiteAuthConfig;
use crate::purge::{OrphanSweep, UserPurgeTable};
use crate::retention::RetentionDecl;
use crate::seed::Seed;
use crate::state::StateRequirement;

pub type AssetFn = Box<dyn Fn(&dyn AssetPaths) -> Vec<AssetDefinition> + Send + Sync>;
pub type RunnerServiceFactory = Box<dyn Factory<Arc<dyn RunnerService>>>;
pub type ReadinessFactory = Box<dyn Factory<Arc<dyn ReadinessCheck>>>;
pub type DiagnosticFactory = Box<dyn Factory<Arc<dyn DiagnosticProbe>>>;
pub type GatewayGuardFactory = Box<dyn Factory<Arc<dyn GatewayRequestGuard>>>;

type ErasedValue = Arc<dyn Any + Send + Sync>;
type ComputeFn = Box<dyn Fn(&Resolved<'_>) -> Result<ErasedValue, ProvideError> + Send + Sync>;
type InsertFn = fn(&mut Dependencies, &ExtensionId, ErasedValue) -> Result<(), DuplicateDependency>;

/// One deferred state value: its type, the provide fn and the monomorphised
/// insert into the extension state.
pub struct ProvideEntry {
    type_id: TypeId,
    type_name: &'static str,
    compute: ComputeFn,
    insert: InsertFn,
}

impl ProvideEntry {
    pub(crate) fn new<T, F>(f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
            compute: Box::new(move |r| {
                f(r).map(|value| {
                    let erased: ErasedValue = Arc::new(value);
                    erased
                })
            }),
            insert: insert_provided::<T>,
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn compute(&self, r: &Resolved<'_>) -> Result<ProvidedValue, ProvideError> {
        (self.compute)(r).map(|value| ProvidedValue {
            value,
            insert: self.insert,
        })
    }
}

impl std::fmt::Debug for ProvideEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvideEntry")
            .field("type", &self.type_name)
            .finish_non_exhaustive()
    }
}

/// A computed state value, held until it is inserted under its provider.
pub struct ProvidedValue {
    value: ErasedValue,
    insert: InsertFn,
}

impl ProvidedValue {
    pub fn insert_into(
        self,
        state: &mut Dependencies,
        provider: &ExtensionId,
    ) -> Result<(), DuplicateDependency> {
        (self.insert)(state, provider, self.value)
    }
}

impl std::fmt::Debug for ProvidedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvidedValue").finish_non_exhaustive()
    }
}

fn insert_provided<T: Send + Sync + 'static>(
    state: &mut Dependencies,
    provider: &ExtensionId,
    value: ErasedValue,
) -> Result<(), DuplicateDependency> {
    match value.downcast::<T>() {
        Ok(typed) => state.provide(provider, typed),
        Err(_) => Ok(()),
    }
}

/// A declared dependency on another member of the same capability, by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberDependency {
    type_id: TypeId,
    type_name: &'static str,
}

impl MemberDependency {
    pub(crate) fn of<E: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<E>(),
            type_name: type_name::<E>(),
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}

/// Everything one member contributed, in declaration order.
#[derive(Default)]
pub struct ContributionSink {
    pub(crate) schemas: Vec<SchemaDefinition>,
    pub(crate) retirements: Vec<SchemaDefinition>,
    pub(crate) migrations: Vec<Migration>,
    pub(crate) seeds: Vec<Seed>,
    pub(crate) cross_tables: Vec<&'static str>,
    pub(crate) user_purges: Vec<UserPurgeTable>,
    pub(crate) orphan_sweeps: Vec<OrphanSweep>,
    pub(crate) roles: Vec<ExtensionRole>,
    pub(crate) assets: Vec<AssetFn>,
    pub(crate) configs: Vec<ConfigDecl>,
    pub(crate) config_planes: Vec<Arc<dyn ConfigPlane>>,
    pub(crate) site_auth: Vec<SiteAuthConfig>,
    pub(crate) provides: Vec<ProvideEntry>,
    pub(crate) requires: Vec<StateRequirement>,
    pub(crate) depends_on: Vec<MemberDependency>,
    pub(crate) jobs: Vec<Arc<dyn Job>>,
    pub(crate) job_run_scopes: Vec<Arc<dyn JobRunScope>>,
    pub(crate) retention: Vec<RetentionDecl>,
    pub(crate) runner_services: Vec<RunnerServiceFactory>,
    pub(crate) readiness: Vec<ReadinessFactory>,
    pub(crate) diagnostics: Vec<DiagnosticFactory>,
    pub(crate) metrics: Vec<MetricDescriptor>,
    pub(crate) gateway_guards: Vec<GatewayGuardFactory>,
    pub(crate) slots: Vec<SlotRecord>,
    #[cfg(feature = "router")]
    pub(crate) routes: Vec<crate::router::RouteSet>,
    #[cfg(feature = "router")]
    pub(crate) gateway_routes: Vec<crate::router::GatewayRouteDecl>,
    #[cfg(feature = "cli")]
    pub(crate) commands: Vec<crate::cli::CommandDecl>,
    pub(crate) errors: Vec<ContributionError>,
}

impl ContributionSink {
    #[must_use]
    pub fn schemas(&self) -> &[SchemaDefinition] {
        &self.schemas
    }

    #[must_use]
    pub fn retirements(&self) -> &[SchemaDefinition] {
        &self.retirements
    }

    #[must_use]
    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    #[must_use]
    pub fn seeds(&self) -> &[Seed] {
        &self.seeds
    }

    #[must_use]
    pub fn cross_tables(&self) -> &[&'static str] {
        &self.cross_tables
    }

    #[must_use]
    pub fn user_purges(&self) -> &[UserPurgeTable] {
        &self.user_purges
    }

    #[must_use]
    pub fn orphan_sweeps(&self) -> &[OrphanSweep] {
        &self.orphan_sweeps
    }

    #[must_use]
    pub fn roles(&self) -> &[ExtensionRole] {
        &self.roles
    }

    #[must_use]
    pub fn assets(&self) -> &[AssetFn] {
        &self.assets
    }

    #[must_use]
    pub fn configs(&self) -> &[ConfigDecl] {
        &self.configs
    }

    #[must_use]
    pub fn config_planes(&self) -> &[Arc<dyn ConfigPlane>] {
        &self.config_planes
    }

    #[must_use]
    pub fn site_auth(&self) -> &[SiteAuthConfig] {
        &self.site_auth
    }

    #[must_use]
    pub fn provides(&self) -> &[ProvideEntry] {
        &self.provides
    }

    #[must_use]
    pub fn requires(&self) -> &[StateRequirement] {
        &self.requires
    }

    #[must_use]
    pub fn depends_on(&self) -> &[MemberDependency] {
        &self.depends_on
    }

    #[must_use]
    pub fn jobs(&self) -> &[Arc<dyn Job>] {
        &self.jobs
    }

    #[must_use]
    pub fn job_run_scopes(&self) -> &[Arc<dyn JobRunScope>] {
        &self.job_run_scopes
    }

    #[must_use]
    pub fn retention(&self) -> &[RetentionDecl] {
        &self.retention
    }

    #[must_use]
    pub fn runner_services(&self) -> &[RunnerServiceFactory] {
        &self.runner_services
    }

    #[must_use]
    pub fn readiness(&self) -> &[ReadinessFactory] {
        &self.readiness
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[DiagnosticFactory] {
        &self.diagnostics
    }

    #[must_use]
    pub fn metrics(&self) -> &[MetricDescriptor] {
        &self.metrics
    }

    #[must_use]
    pub fn gateway_guards(&self) -> &[GatewayGuardFactory] {
        &self.gateway_guards
    }

    #[must_use]
    pub fn slots(&self) -> &[SlotRecord] {
        &self.slots
    }

    #[cfg(feature = "router")]
    #[must_use]
    pub fn routes(&self) -> &[crate::router::RouteSet] {
        &self.routes
    }

    #[cfg(feature = "router")]
    #[must_use]
    pub fn gateway_routes(&self) -> &[crate::router::GatewayRouteDecl] {
        &self.gateway_routes
    }

    #[cfg(feature = "router")]
    #[doc(hidden)]
    pub fn take_routes(
        &mut self,
    ) -> (
        Vec<crate::router::RouteSet>,
        Vec<crate::router::GatewayRouteDecl>,
    ) {
        (
            std::mem::take(&mut self.routes),
            std::mem::take(&mut self.gateway_routes),
        )
    }

    #[cfg(feature = "cli")]
    #[must_use]
    pub fn commands(&self) -> &[crate::cli::CommandDecl] {
        &self.commands
    }

    #[cfg(feature = "cli")]
    #[doc(hidden)]
    pub fn take_commands(&mut self) -> Vec<crate::cli::CommandDecl> {
        std::mem::take(&mut self.commands)
    }

    #[doc(hidden)]
    pub fn take_errors(&mut self) -> Vec<ContributionError> {
        std::mem::take(&mut self.errors)
    }
}

impl std::fmt::Debug for ContributionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContributionSink")
            .field("schemas", &self.schemas.len())
            .field("provides", &self.provides)
            .field("jobs", &self.jobs.len())
            .field("slots", &self.slots)
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}
