//! The slim runtime kernel the admitted composition resolves against.
//!
//! A [`Kernel`] holds the database handle, the process's background-task
//! owner, the kernel handles a conferral can hand a route set, and the
//! extension state. [`Kernel::resolve`] seeds that state with what the
//! admitted members declared: their projection drains
//! ([`AdmittedProjections`]), their agent-run origins ([`AdmittedRunOrigins`])
//! their owner-stamped purge tables and orphan sweeps in composition order
//! ([`AdmittedPurge`]) and their jobs, retention work and job-run scopes
//! ([`AdmittedSchedule`]), and the non-secret views a member's command reads
//! in place of the composition: declared migrations ([`AdmittedMigrations`]),
//! configuration sections ([`AdmittedConfigs`]) and asset declarations
//! ([`AdmittedAssets`]), each open. It then builds the state from
//! the [`Admitted`] handoff: every admitted member's early provide fns in
//! composition order, each reading what was provided before it; the admitted
//! owner reassignments ([`AdmittedOwnerReassignments`], sealed: kernel code
//! and provide fns read them, no member runtime context does), each built by
//! its owner's factory and refused unless it names only the owner's own
//! tables;
//! every slot,
//! built from its admitted entries (core first where the slot asks for it)
//! and inserted as `Arc<S::Built>`, after the shared audience-resolver set
//! ([`AudienceResolvers`]) every audience-reading slot consults; the governance
//! surfaces every member reads by type — the [`SubjectProviderSet`], the
//! [`PolicyCatalog`] and the
//! [`GatewayGuards`](systemprompt_extension::GatewayGuards) — each inserted as
//! an `Arc`, a reserved or doubly declared subject dimension refusing the
//! resolution; then every member's provide fns declared to run after
//! governance, which may read the slots and governance surfaces; and last
//! every member's declared requirements, checked against
//! the finished state, so a member may require a slot's built value or a
//! governance surface. The values the kernel inserts after the provide fns run
//! — the audience resolvers, the owner reassignments and the three governance
//! surfaces — are reserved
//! before any member's code runs: a member that provides one, or opens a slot
//! that builds one, is refused with [`KernelError::ReservedType`]; a member
//! that provides a value under a conferral row which does not name it, or
//! that lacks core standing, is refused with
//! [`KernelError::UnlistedConferral`]. Only
//! admitted members' entries are present, so nothing a disabled member
//! contributed runs.
//!
//! Every member's provide fns, slot entries and governance factories run
//! with that member's admitted standing ([`Kernel::member_resolved`]): a
//! member with install standing reads only the host-sealed seeds the closed
//! `reserved::INSTALL_SEEDS` table lists and none of another member's sealed
//! provides, so a forged install provide asking for the messaging
//! dispatcher, the spawner or the signing keys fails the resolution with
//! [`KernelError::Provide`] before any of its values exists.
//!
//! [`Kernel::runtime_state`] is the one base every context handed to member
//! runtime code is built from — a job, a runner service and a config plane:
//! the resolved state with every sealed seed and every sealed provide
//! withheld. [`Kernel::route_scoped`] resolves a route set's state for its
//! owner, which is refused every host-sealed seed and a value another member
//! provided sealed.
//!
//! [`Kernel::detached`] holds a seeded state without a database and without
//! resolving any member: a CLI command whose descriptor needs no database
//! reads the profile-level seeds through it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_agent_runs::origin::AdmittedRunOrigins;
use systemprompt_extension::contribute::{PendingBind, SlotBuildInput, build_slot};
use systemprompt_extension::{
    Admitted, AdmittedAssets, AdmittedConfigs, AdmittedMigrations, AdmittedOwnerReassignments,
    AdmittedProjections, AdmittedPurge, AdmittedSchedule, AudienceResolvers, BuiltState, HandleId,
    KernelHandle, MemberResolved, MemberScope, ProvideError, ProvidePhase, Resolved, StandingTag,
};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::policy::PolicyCatalog;
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

use self::reserved::{gateway_guards, refuse_kernel_types, refuse_unlisted_conferrals};

mod error;
mod factories;
mod reserved;
mod secrets;

pub use error::KernelError;

pub struct Kernel {
    pub(crate) database: Arc<dyn DatabaseHandle>,
    pub(crate) tasks: BackgroundTasks,
    pub(crate) handles: HashMap<HandleId, Arc<dyn Any + Send + Sync>>,
    state: Dependencies,
}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kernel")
            .field("handles", &self.handles.keys().collect::<Vec<_>>())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Kernel {
    #[must_use]
    pub fn new(
        database: Arc<dyn DatabaseHandle>,
        tasks: BackgroundTasks,
        seed: Dependencies,
    ) -> Self {
        Self {
            database,
            tasks,
            handles: HashMap::new(),
            state: seed,
        }
    }

    #[must_use]
    pub fn detached(tasks: BackgroundTasks, seed: Dependencies) -> Self {
        Self::new(Arc::new(DetachedDatabase), tasks, seed)
    }

    #[must_use]
    pub fn with_handle<K: KernelHandle>(mut self, value: K::Value) -> Self {
        self.handles.insert(K::ID, Arc::new(value));
        self
    }

    pub fn resolve(&mut self, admitted: &Admitted) -> Result<(), KernelError> {
        self.state.insert(AdmittedProjections::of(admitted.built()));
        self.state.insert(AdmittedRunOrigins::of(admitted.built()));
        self.state.insert(AdmittedPurge::of(admitted.built())?);
        self.state.insert(AdmittedSchedule::of(admitted.built())?);
        self.state.insert(AdmittedMigrations::of(admitted.built()));
        self.state.insert(AdmittedConfigs::of(admitted.built()));
        self.state.insert(AdmittedAssets::of(admitted.built()));
        refuse_kernel_types(admitted.built())?;
        refuse_unlisted_conferrals(admitted.built())?;
        let pending = self.seed_late_cells(admitted)?;
        let declarations =
            secrets::declarations(admitted.built(), &Resolved::__new(&*self, &self.state))?;
        self.state.insert_sealed(Arc::new(declarations));
        self.provide_all(admitted, ProvidePhase::Early)?;
        let audiences = AudienceResolvers::from_admitted(admitted.built(), &self.resolved())?;
        self.state.insert(Arc::new(audiences));
        let reassignments = AdmittedOwnerReassignments::of(admitted.built())?;
        self.state.insert_sealed(Arc::new(reassignments));
        self.build_slots(admitted, false)?;
        self.build_governance(admitted.built())?;
        self.provide_all(admitted, ProvidePhase::AfterGovernance)?;
        self.build_slots(admitted, true)?;
        self.check_requirements(admitted)?;
        for bind in pending {
            bind(&self.state)?;
        }
        Ok(())
    }

    fn seed_late_cells(&mut self, admitted: &Admitted) -> Result<Vec<PendingBind>, KernelError> {
        let mut pending = Vec::new();
        for member in admitted.built().members() {
            for cell in member.sink.late_cells() {
                if member.standing != StandingTag::Core {
                    return Err(KernelError::InstallLateCell {
                        owner: member.owner.clone(),
                        ty: cell.type_name(),
                    });
                }
                pending.push(cell.seed_into(&mut self.state));
            }
        }
        Ok(pending)
    }

    #[must_use]
    pub fn resolved(&self) -> Resolved<'_> {
        Resolved::__new(self, &self.state)
    }

    #[must_use]
    pub fn member_resolved<'a>(&'a self, scope: &'a MemberScope) -> MemberResolved<'a> {
        Resolved::__member_scoped(self, &self.state, scope)
    }

    #[must_use]
    pub fn route_scoped<'a>(
        &'a self,
        owner: &'a ExtensionId,
        standing: StandingTag,
        conferred: &'a [HandleId],
    ) -> Resolved<'a> {
        Resolved::__route_scoped(self, &self.state, owner, standing, conferred)
    }

    #[must_use]
    pub const fn state(&self) -> &Dependencies {
        &self.state
    }

    #[must_use]
    pub fn runtime_state(&self) -> Dependencies {
        self.state.clone().withholding_sealed()
    }

    fn provide_all(&mut self, admitted: &Admitted, phase: ProvidePhase) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            for entry in member
                .sink
                .provides()
                .iter()
                .filter(|entry| entry.phase() == phase)
            {
                let scope = member.scope();
                match entry.compute(&self.member_resolved(&scope)) {
                    Ok(value) => value.insert_into(&mut self.state, &member.owner)?,
                    Err(ProvideError::Withheld) => entry.withhold_into(&mut self.state),
                    Err(source) => {
                        return Err(KernelError::Provide {
                            owner: member.owner.clone(),
                            ty: entry.type_name(),
                            source,
                        });
                    },
                }
            }
        }
        Ok(())
    }

    fn check_requirements(&self, admitted: &Admitted) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            if let Some(unmet) = member
                .sink
                .requires()
                .iter()
                .find(|requirement| !requirement.is_met_by(&self.state))
            {
                return Err(KernelError::Missing {
                    owner: member.owner.clone(),
                    ty: unmet.type_name(),
                });
            }
        }
        Ok(())
    }

    fn build_slots(&mut self, admitted: &Admitted, late: bool) -> Result<(), KernelError> {
        let mut order: Vec<TypeId> = Vec::new();
        let mut grouped: HashMap<TypeId, Vec<SlotBuildInput>> = HashMap::new();
        for member in admitted.built().members() {
            for record in member
                .sink
                .slots()
                .iter()
                .filter(|record| record.late() == late)
            {
                let input = record
                    .take_entry(member.owner.clone(), member.standing)
                    .ok_or_else(|| KernelError::AlreadyResolved {
                        owner: member.owner.clone(),
                    })?;
                if !grouped.contains_key(&input.slot()) {
                    order.push(input.slot());
                }
                grouped.entry(input.slot()).or_default().push(input);
            }
        }
        for slot in order {
            let inputs = grouped.remove(&slot).unwrap_or_default();
            let built = build_slot(inputs, &Resolved::__new(&*self, &self.state))?;
            if let Some(built) = built {
                built.insert_into(&mut self.state)?;
            }
        }
        Ok(())
    }

    fn build_governance(&mut self, built: &BuiltState) -> Result<(), KernelError> {
        let r = Resolved::__new(&*self, &self.state);
        let subjects = SubjectProviderSet::from_admitted(built, &r)?;
        let policies = PolicyCatalog::from_admitted(built, &r)?;
        let guards = gateway_guards(built, &r)?;
        self.state.insert(Arc::new(subjects));
        self.state.insert(Arc::new(policies));
        self.state.insert(Arc::new(guards));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct DetachedDatabase;

impl DatabaseHandle for DetachedDatabase {
    fn is_connected(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}
