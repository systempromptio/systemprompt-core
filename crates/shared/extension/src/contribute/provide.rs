//! The values a member provides to the kernel state.
//!
//! A value a member provides with [`Contributions::provide`] is provided on
//! every host that admits the member: there is no conditional provide.
//! [`Contributions::provide_after_governance`] provides the same way, after
//! every slot and the governance surfaces (subject providers, policy catalog,
//! gateway guards) are built, for a value assembled from them. A provide fn
//! reads its dependencies with `r.get`, and a missing dependency fails the
//! boot; a consumer that may run without a value reads it with `r.optional`.
//! A provide fn whose value cannot exist on this host (it is built from a
//! seed only the API host holds) returns `ProvideError::Withheld`: the kernel
//! reserves the type instead, so no member can provide a substitute, and every
//! `get` of it is a missing dependency marked withheld, which a dependent
//! provide fn's `?` turns into `ProvideError::Withheld`: the dependent is
//! withheld too, transitively. A type nothing provides stays a plain missing
//! dependency and fails the boot.
//!
//! [`Contributions::provide_sealed`] and
//! [`Contributions::provide_sealed_after_governance`] provide a value that
//! acts with standing authority (built from a sealed seed, bound to the system
//! admin, or carrying secret-derived material). It is sealed exactly as a
//! kernel seed is: the kernel, provide fns, factories and slot builds read it;
//! a job, runner-service or config-plane context, a member's command and the
//! route state of any member but its provider are refused it as missing.
//! [`Contributions::provide_conferred`] provides it sealed and confers it on
//! the one core holder its reserved row names (`reserved::SEALED_CONFERRALS`).
//! [`Contributions::late_cell`] is not a provide either: it declares a cell
//! the kernel seeds and binds (`contribute::LateCell`).
//! [`Contributions::admin_bootstrap`] is not a provide: it stores the
//! pre-kernel admin-bootstrap constructor, which the CLI host calls only for a
//! core-standing `users` member.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::entries::{ProvideEntry, ProvidePhase};
use super::resolved::ProvideFn;
use super::{AdminBootstrapFn, Contributions};
use crate::reserved::ConferralRow;

impl Contributions<'_> {
    pub fn admin_bootstrap(&mut self, f: AdminBootstrapFn) -> &mut Self {
        self.sink.admin_bootstrap = Some(f);
        self
    }

    pub fn domain_config(&mut self, f: super::DomainConfigFn) -> &mut Self {
        self.sink.domain_configs.push(f);
        self
    }

    pub fn late_cell<C: super::LateCell>(&mut self) -> &mut Self {
        self.sink.late_cells.push(super::LateCellEntry::of::<C>());
        self
    }

    pub fn provide<T, F>(&mut self, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.push_provide(ProvidePhase::Early, false, f)
    }

    pub fn provide_sealed<T, F>(&mut self, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.push_provide(ProvidePhase::Early, true, f)
    }

    pub fn provide_after_governance<T, F>(&mut self, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.push_provide(ProvidePhase::AfterGovernance, false, f)
    }

    pub fn provide_sealed_after_governance<T, F>(&mut self, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.push_provide(ProvidePhase::AfterGovernance, true, f)
    }

    pub fn provide_conferred<T, F>(&mut self, row: ConferralRow, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.sink
            .provides
            .push(ProvideEntry::conferred::<T, F>(ProvidePhase::Early, row, f));
        self
    }

    pub fn provide_conferred_after_governance<T, F>(&mut self, row: ConferralRow, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.sink.provides.push(ProvideEntry::conferred::<T, F>(
            ProvidePhase::AfterGovernance,
            row,
            f,
        ));
        self
    }

    fn push_provide<T, F>(&mut self, phase: ProvidePhase, sealed: bool, f: F) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
        F: ProvideFn<T>,
    {
        self.sink
            .provides
            .push(ProvideEntry::new::<T, F>(phase, sealed, f));
        self
    }
}
