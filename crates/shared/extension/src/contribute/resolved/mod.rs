//! The read side of composition: what a provide fn, a factory, a slot build,
//! a command and a route set's state resolve their dependencies from.
//!
//! A [`Resolved`] is opaque. It reads the built extension state by type
//! ([`Resolved::get`], [`Resolved::optional`]) and hands out the kernel's
//! database handle and background-task owner; it is built only by the
//! runtime's single [`KernelAccess`] implementation and, for a route set, by
//! the API mount.
//!
//! A seed the host inserted sealed (secrets, the runtime `Config`, boot-only
//! data) is served to kernel code and to the provide fns, factories, slot
//! builds and governance factories of a member with core standing. The
//! kernel resolves each member's code with that member's admitted standing
//! ([`Resolved::__member_scoped`], [`MemberResolved::within`]): a member with
//! install standing reads a host-sealed seed only when the closed
//! `reserved::INSTALL_SEEDS` table lists its type, and never a value another
//! member provided sealed. Rescoping never widens: a resolution scoped to an
//! install stays scoped to that install whatever it is rescoped to. The
//! resolver a member's command runs against ([`Resolved::__member_command`])
//! refuses every sealed value with the same `MissingDependency` an absent
//! type gets; resolved for the command's owner and its admitted standing
//! ([`Resolved::__member_command_owned`]) it serves the one exception below,
//! a value recorded under a conferral row whose core holder is that owner, and
//! nothing else sealed, so an install command reads no conferred value. A value
//! a member provided from a sealed seed is the member's own and stays readable;
//! a value a member provided sealed is withheld from that resolver too, and
//! from the state of a route set another member owns
//! ([`Resolved::__route_scoped`] names the set's owner and its admitted
//! standing). No route set's state reads a host-sealed seed, whatever its
//! owner's standing: a route set reads what its member built in a provide fn.
//! A member value or a host seed recorded under a reserved conferral row
//! (`reserved::SEALED_CONFERRALS`; a host seed's provider is the kernel) is
//! the one exception: it is served to the state of the route set whose owner
//! is that row's core holder, with core standing; the row is re-checked
//! against the provider, the holder and the standing on every read.
//!
//! A kernel handle is not reachable through a public method. It is read only
//! through [`Handed<K>`](super::handles::Handed)'s [`FromState`] impl, which
//! consults the resolution's private conferred-handle list. The list is empty
//! for every resolution but the one the mount builds for a route set whose base
//! carries a `KernelHandle(K::ID)` conferral for that set's owner, so a provide
//! fn, a factory, a slot build, a command or a route set at an unconferred base
//! gets `MissingDependency`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DependencyProvider, MissingDependency};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

use super::handles::{HandleId, KernelHandle};
use super::member::{MemberResolved, MemberScope};
use super::standing::StandingTag;
use crate::reserved::{CoreId, conferral_admits};

use self::withhold::{install_withholds, provider_id};

mod state_fn;
mod withhold;

pub use state_fn::{FromState, ProvideFn};

/// What the kernel exposes to a resolution; implemented once, in the runtime.
#[doc(hidden)]
pub trait KernelAccess: Send + Sync {
    fn database(&self) -> Arc<dyn DatabaseHandle>;
    fn tasks(&self) -> &BackgroundTasks;
    fn handle(&self, id: HandleId) -> Option<Arc<dyn Any + Send + Sync>>;
}

pub struct Resolved<'a> {
    access: &'a dyn KernelAccess,
    state: &'a Dependencies,
    conferred: &'a [HandleId],
    withhold_sealed: bool,
    viewer: Option<RouteViewer<'a>>,
    commander: Option<RouteViewer<'a>>,
    scope: Option<RouteViewer<'a>>,
}

#[derive(Debug, Clone, Copy)]
struct RouteViewer<'a> {
    owner: &'a ExtensionId,
    standing: StandingTag,
}

impl<'a> Resolved<'a> {
    #[doc(hidden)]
    pub const fn __new(access: &'a dyn KernelAccess, state: &'a Dependencies) -> Self {
        Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: false,
            viewer: None,
            commander: None,
            scope: None,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn __member_scoped(
        access: &'a dyn KernelAccess,
        state: &'a Dependencies,
        scope: &'a MemberScope,
    ) -> MemberResolved<'a> {
        MemberResolved::from_scoped(Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: false,
            viewer: None,
            commander: None,
            scope: Some(RouteViewer {
                owner: scope.owner(),
                standing: scope.standing(),
            }),
        })
    }

    #[doc(hidden)]
    pub const fn __member_command(access: &'a dyn KernelAccess, state: &'a Dependencies) -> Self {
        Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: true,
            viewer: None,
            commander: None,
            scope: None,
        }
    }

    #[doc(hidden)]
    pub const fn __member_command_owned(
        access: &'a dyn KernelAccess,
        state: &'a Dependencies,
        owner: &'a ExtensionId,
        standing: StandingTag,
    ) -> Self {
        Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: true,
            viewer: None,
            commander: Some(RouteViewer { owner, standing }),
            scope: None,
        }
    }

    #[doc(hidden)]
    pub const fn __route_scoped(
        access: &'a dyn KernelAccess,
        state: &'a Dependencies,
        owner: &'a ExtensionId,
        standing: StandingTag,
        conferred: &'a [HandleId],
    ) -> Self {
        Self {
            access,
            state,
            conferred,
            withhold_sealed: false,
            viewer: Some(RouteViewer { owner, standing }),
            commander: None,
            scope: None,
        }
    }

    pub(crate) fn scoped_to<'b>(
        &self,
        owner: &'b ExtensionId,
        standing: StandingTag,
    ) -> Resolved<'b>
    where
        'a: 'b,
    {
        let scope = match self.scope {
            Some(current) if current.standing == StandingTag::Install => current,
            _ => RouteViewer { owner, standing },
        };
        Resolved {
            access: self.access,
            state: self.state,
            conferred: self.conferred,
            withhold_sealed: self.withhold_sealed,
            viewer: self.viewer,
            commander: self.commander,
            scope: Some(scope),
        }
    }
}

impl Resolved<'_> {
    pub fn get<T: Clone + Send + Sync + 'static>(&self) -> Result<T, MissingDependency> {
        self.optional::<T>().ok_or_else(|| {
            if self.state.is_withheld::<T>() {
                MissingDependency::withheld::<T>()
            } else {
                MissingDependency::of::<T>()
            }
        })
    }

    pub fn optional<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        if self.state.is_sealed::<T>() && self.withholds_sealed::<T>() {
            return None;
        }
        self.state.find::<T>().cloned()
    }

    fn withholds_sealed<T: Send + Sync + 'static>(&self) -> bool {
        let provider = self.state.provider_of::<T>();
        if self.withhold_sealed {
            return !self.commander.is_some_and(|commander| {
                provider_id(provider)
                    .is_some_and(|provider| self.conferred_on_viewer::<T>(provider, commander))
            });
        }
        if let Some(scope) = self.scope
            && scope.standing == StandingTag::Install
            && install_withholds::<T>(provider, scope.owner)
        {
            return true;
        }
        let Some(viewer) = self.viewer else {
            return false;
        };
        match provider {
            Some(DependencyProvider::Extension(provider)) => {
                provider != viewer.owner
                    && !self.conferred_on_viewer::<T>(provider.as_str(), viewer)
            },
            Some(DependencyProvider::Host) => {
                !self.conferred_on_viewer::<T>(CoreId::Kernel.id(), viewer)
            },
            None => true,
        }
    }

    fn conferred_on_viewer<T: Send + Sync + 'static>(
        &self,
        provider: &str,
        viewer: RouteViewer<'_>,
    ) -> bool {
        viewer.standing == StandingTag::Core
            && self.state.conferral_of::<T>().is_some_and(|conferred| {
                &conferred.holder == viewer.owner
                    && conferral_admits(conferred.row, provider, viewer.owner.as_str())
            })
    }

    pub(super) fn secret_reader(&self) -> Option<(&ExtensionId, &Dependencies)> {
        if self.withhold_sealed || self.viewer.is_some() {
            return None;
        }
        self.scope.map(|scope| (scope.owner, self.state))
    }

    pub fn db(&self) -> Arc<dyn DatabaseHandle> {
        self.access.database()
    }

    pub fn tasks(&self) -> &BackgroundTasks {
        self.access.tasks()
    }

    pub(super) fn handed<K: KernelHandle>(&self) -> Result<K::Value, MissingDependency> {
        if !self.conferred.contains(&K::ID) {
            return Err(MissingDependency::of::<K::Value>());
        }
        self.access
            .handle(K::ID)
            .and_then(|value| value.downcast_ref::<K::Value>().cloned())
            .ok_or_else(MissingDependency::of::<K::Value>)
    }
}

impl std::fmt::Debug for Resolved<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("state", self.state)
            .field("conferred", &self.conferred)
            .field("withhold_sealed", &self.withhold_sealed)
            .field("viewer", &self.viewer)
            .field("commander", &self.commander)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
