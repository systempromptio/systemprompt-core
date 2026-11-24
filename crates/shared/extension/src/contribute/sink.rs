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

use std::sync::Arc;

use systemprompt_provider_contracts::ConfigPlane;
use systemprompt_traits::{JobRunScope, RunOrigin, SecretPattern};

use super::config::ConfigDecl;
use super::entries::{ContributedJob, MemberDependency, ProvideEntry};
use super::error::ContributionError;
use super::slot::SlotRecord;
use crate::health::MetricDescriptor;
use crate::metadata::{ExtensionRole, SchemaDefinition};
use crate::migration::Migration;
use crate::projection::ProjectionEntry;
use crate::protection::SiteAuthConfig;
use crate::purge::{OrphanSweep, UserPurgeTable};
use crate::reassignment::OwnerReassignment;
use crate::retention::RetentionDecl;
use crate::seed::Seed;
use systemprompt_provider_contracts::StateRequirement;

#[cfg(feature = "sqlx")]
use super::factories::DoctorProbeSourceRef;
use super::factories::SecretReferenceFactory;
pub use super::factories::{
    AdminBootstrapFn, AssetFn, AudienceResolverFactory, DiagnosticFactory, DoctorCheckRef,
    GatewayGuardFactory, PolicyFactory, ReadinessFactory, RunnerServiceFactory,
    SubjectProviderFactory,
};

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
    pub(crate) owner_reassignments: Vec<OwnerReassignment>,
    pub(crate) doctor_checks: Vec<DoctorCheckRef>,
    #[cfg(feature = "sqlx")]
    pub(crate) doctor_probe_sources: Vec<DoctorProbeSourceRef>,
    pub(crate) roles: Vec<ExtensionRole>,
    pub(crate) assets: Vec<AssetFn>,
    pub(crate) configs: Vec<ConfigDecl>,
    pub(crate) config_planes: Vec<Arc<dyn ConfigPlane>>,
    pub(crate) admin_bootstrap: Option<AdminBootstrapFn>,
    pub(crate) site_auth: Vec<SiteAuthConfig>,
    pub(crate) provides: Vec<ProvideEntry>,
    pub(crate) requires: Vec<StateRequirement>,
    pub(crate) depends_on: Vec<MemberDependency>,
    pub(crate) jobs: Vec<ContributedJob>,
    pub(crate) job_run_scopes: Vec<Arc<dyn JobRunScope>>,
    pub(crate) retention: Vec<RetentionDecl>,
    pub(crate) projections: Vec<ProjectionEntry>,
    pub(crate) run_origins: Vec<RunOrigin>,
    pub(crate) runner_services: Vec<RunnerServiceFactory>,
    pub(crate) readiness: Vec<ReadinessFactory>,
    pub(crate) diagnostics: Vec<DiagnosticFactory>,
    pub(crate) metrics: Vec<MetricDescriptor>,
    pub(crate) gateway_guards: Vec<GatewayGuardFactory>,
    pub(crate) subject_providers: Vec<SubjectProviderFactory>,
    pub(crate) audience_resolvers: Vec<AudienceResolverFactory>,
    pub(crate) policies: Vec<PolicyFactory>,
    pub(crate) declared_secrets: Vec<SecretPattern>,
    pub(crate) secret_references: Vec<SecretReferenceFactory>,
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
    pub fn owner_reassignments(&self) -> &[OwnerReassignment] {
        &self.owner_reassignments
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
    pub const fn admin_bootstrap(&self) -> Option<AdminBootstrapFn> {
        self.admin_bootstrap
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
    pub fn jobs(&self) -> &[ContributedJob] {
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
    pub fn projections(&self) -> &[ProjectionEntry] {
        &self.projections
    }

    #[must_use]
    pub fn run_origins(&self) -> &[RunOrigin] {
        &self.run_origins
    }

    #[must_use]
    pub fn subject_providers(&self) -> &[SubjectProviderFactory] {
        &self.subject_providers
    }

    pub fn audience_resolvers(&self) -> &[AudienceResolverFactory] {
        &self.audience_resolvers
    }

    #[must_use]
    pub fn policies(&self) -> &[PolicyFactory] {
        &self.policies
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
            .field("jobs", &self.jobs)
            .field("slots", &self.slots)
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}
