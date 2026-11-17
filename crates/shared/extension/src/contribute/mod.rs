//! How a member declares what it contributes.
//!
//! [`Extension::contribute`] receives a
//! [`Contributions`] bound to the member's own [`ContributionSink`]. Every
//! method only stores: a value passed by value (`job`, `config_plane`, a
//! retention hold) must be inert until admitted, and provide fns, factories,
//! slot builds and asset closures are deferred. Nothing a member contributes
//! runs before the composition's admission has dropped every entry of a
//! disabled member.
//!
//! No method returns or accepts another member's entries; the owner of every
//! entry is the member whose sink holds it.
//!
//! A value a member provides with [`Contributions::provide`] is provided on
//! every host that admits the member: there is no conditional provide. A
//! provide fn reads its dependencies with `r.get`, and a missing dependency
//! fails the boot; a consumer that may run without a value reads it with
//! `r.optional`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audience;
mod config;
mod entries;
mod error;
pub mod handles;
mod resolved;
mod sink;
mod slot;
mod standing;
mod view;

use std::any::TypeId;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::governance::GovernancePolicy;
use systemprompt_provider_contracts::{ConfigPlane, Job};
use systemprompt_traits::authz::SubjectAttributeProvider;
use systemprompt_traits::{JobRunScope, RunOrigin, RunnerService};

pub use audience::{AudienceEntry, AudienceResolvers};
pub use config::{ConfigDecl, ExtensionConfig};
pub use entries::{ContributedJob, MemberDependency, ProvideEntry, ProvidedValue};
pub use error::{ContributionError, FactoryError, ProvideError, SlotError};
pub use resolved::{
    Factory, FromState, Handed, HandleId, KernelAccess, KernelHandle, ProvideFn, Resolved,
    build_factories,
};
pub use sink::{
    AssetFn, AudienceResolverFactory, ContributionSink, DiagnosticFactory, GatewayGuardFactory,
    PolicyFactory, ReadinessFactory, RunnerServiceFactory, SubjectProviderFactory,
};
pub use slot::{Slot, SlotBuildInput, SlotBuilt, SlotEntry, SlotOrder, SlotRecord, build_slot};
pub use standing::StandingTag;
pub use view::KernelView;

use crate::Extension;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::gateway_guard::GatewayRequestGuard;
use crate::health::{DiagnosticProbe, MetricDescriptor, ReadinessCheck};
use crate::metadata::{ExtensionRole, SchemaDefinition};
use crate::migration::Migration;
use crate::projection::{DrainTarget, ProjectionDrain, ProjectionEntry};
use crate::protection::SiteAuthConfig;
use crate::purge::{OrphanSweep, UserPurgeTable};
use crate::retention::RetentionDecl;
use crate::seed::Seed;
use crate::state::StateRequirement;

/// A member's sink for its declarations; every value it provides is provided
/// on every host that admits the member.
pub struct Contributions<'a> {
    owner: ExtensionId,
    view: KernelView<'a>,
    sink: &'a mut ContributionSink,
}

impl<'a> Contributions<'a> {
    #[doc(hidden)]
    pub const fn __new(owner: ExtensionId, sink: &'a mut ContributionSink) -> Self {
        Self {
            owner,
            view: KernelView::__new(),
            sink,
        }
    }

    pub const fn kernel(&self) -> &KernelView<'a> {
        &self.view
    }

    pub fn schema(&mut self, s: SchemaDefinition) -> &mut Self {
        self.sink.schemas.push(s);
        self
    }

    pub fn retire(&mut self, s: SchemaDefinition) -> &mut Self {
        self.sink.retirements.push(s);
        self
    }

    pub fn migrations(&mut self, m: Vec<Migration>) -> &mut Self {
        self.sink.migrations.extend(m);
        self
    }

    pub fn seed(&mut self, s: Seed) -> &mut Self {
        self.sink.seeds.push(s);
        self
    }

    pub fn cross_table(&mut self, table: &'static str) -> &mut Self {
        self.sink.cross_tables.push(table);
        self
    }

    pub fn user_purge(&mut self, p: UserPurgeTable) -> &mut Self {
        self.sink.user_purges.push(p);
        self
    }

    pub fn orphan_sweep(&mut self, s: OrphanSweep) -> &mut Self {
        self.sink.orphan_sweeps.push(s);
        self
    }

    pub fn role(&mut self, r: ExtensionRole) -> &mut Self {
        self.sink.roles.push(r);
        self
    }

    pub fn asset_with<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&dyn AssetPaths) -> Vec<AssetDefinition> + Send + Sync + 'static,
    {
        self.sink.assets.push(Arc::new(f));
        self
    }

    pub fn config<C: ExtensionConfig>(&mut self) -> &mut Self {
        self.sink.configs.push(ConfigDecl::of::<C>());
        self
    }

    pub fn config_plane(&mut self, p: Arc<dyn ConfigPlane>) -> &mut Self {
        self.sink.config_planes.push(p);
        self
    }

    pub fn site_auth(&mut self, s: SiteAuthConfig) -> &mut Self {
        self.sink.site_auth.push(s);
        self
    }

    pub fn provide<T, F>(&mut self, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.sink.provides.push(ProvideEntry::new::<T, F>(f));
        self
    }

    pub fn requires<T: Send + Sync + 'static>(&mut self) -> &mut Self {
        self.sink.requires.push(StateRequirement::of::<T>());
        self
    }

    pub fn depends_on<E: Extension>(&mut self) -> &mut Self {
        self.sink.depends_on.push(MemberDependency::of::<E>());
        self
    }

    pub fn job<J: Job + 'static>(&mut self, j: J) -> &mut Self {
        self.sink.jobs.push(ContributedJob::new(j));
        self
    }

    pub fn job_run_scope(&mut self, s: Arc<dyn JobRunScope>) -> &mut Self {
        self.sink.job_run_scopes.push(s);
        self
    }

    pub fn retention(&mut self, r: RetentionDecl) -> &mut Self {
        self.sink.retention.push(r);
        self
    }

    pub fn projection<J: DrainTarget>(&mut self, d: ProjectionDrain) -> &mut Self {
        self.sink.projections.push(ProjectionEntry::of::<J>(d));
        self
    }

    pub fn run_origin(&mut self, o: RunOrigin) -> &mut Self {
        self.sink.run_origins.push(o);
        self
    }

    pub fn runner_service<F: Factory<Arc<dyn RunnerService>>>(&mut self, f: F) -> &mut Self {
        self.sink.runner_services.push(Box::new(f));
        self
    }

    pub fn readiness<F: Factory<Arc<dyn ReadinessCheck>>>(&mut self, f: F) -> &mut Self {
        self.sink.readiness.push(Box::new(f));
        self
    }

    pub fn diagnostic<F: Factory<Arc<dyn DiagnosticProbe>>>(&mut self, f: F) -> &mut Self {
        self.sink.diagnostics.push(Box::new(f));
        self
    }

    pub fn metric(&mut self, m: MetricDescriptor) -> &mut Self {
        self.sink.metrics.push(m);
        self
    }

    pub fn gateway_guard<F: Factory<Arc<dyn GatewayRequestGuard>>>(&mut self, f: F) -> &mut Self {
        self.sink.gateway_guards.push(Box::new(f));
        self
    }

    pub fn subject_attributes<F>(&mut self, f: F) -> &mut Self
    where
        F: Factory<Arc<dyn SubjectAttributeProvider>>,
    {
        self.sink.subject_providers.push(Box::new(f));
        self
    }
    pub fn policy<F: Factory<Arc<dyn GovernancePolicy>>>(&mut self, f: F) -> &mut Self {
        self.sink.policies.push(Box::new(f));
        self
    }

    pub fn slot<S: Slot>(&mut self) -> &mut S::Builder {
        let slots = &mut self.sink.slots;
        let index = slots
            .iter()
            .position(|record| record.slot() == TypeId::of::<S>())
            .unwrap_or_else(|| {
                slots.push(SlotRecord::new::<S>());
                slots.len() - 1
            });
        slots[index]
            .builder_mut::<S>()
            .unwrap_or_else(|| unreachable!("slot records are keyed by the slot's TypeId"))
    }

    #[cfg(feature = "router")]
    pub fn routes(&mut self, r: crate::router::RouteSet) -> &mut Self {
        let mut r = r;
        if let Some(error) = r.take_error() {
            self.sink.errors.push(error);
        }
        if let base @ crate::router::BaseRef::Gateway(_) = r.base() {
            self.sink.errors.push(ContributionError::RouteSetKind(base));
        }
        self.sink.routes.push(r);
        self
    }

    #[cfg(feature = "router")]
    pub fn gateway_route(&mut self, r: crate::router::GatewayRouteDecl) -> &mut Self {
        let mut r = r;
        if let Some(error) = r.router.take_error() {
            self.sink.errors.push(error);
        }
        if let base @ crate::router::BaseRef::Path(_) = r.router.base() {
            self.sink.errors.push(ContributionError::RouteSetKind(base));
        }
        self.sink.gateway_routes.push(r);
        self
    }

    #[cfg(feature = "cli")]
    pub fn command(&mut self, c: crate::cli::CommandDecl) -> &mut Self {
        self.sink.commands.push(c);
        self
    }
}

impl std::fmt::Debug for Contributions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contributions")
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}
