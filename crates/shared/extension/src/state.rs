//! Typed, extension-scoped state built once at boot.
//!
//! The state is a [`Dependencies`] map — the one type-keyed map the platform
//! hands to every factory, job, config plane and prerender context. The host
//! seeds it with the kernel's own handles (database pool, paths, files
//! configuration, repositories, documents service, notification sink), then
//! [`ExtensionRegistry::build_state`] runs [`Extension::provide`] for every
//! enabled extension in dependency order, handing each a [`StateBuilder`]
//! scoped to it. The builder records which extension provided each type and
//! refuses a second provider of a type that is already present, whether the
//! kernel or another extension put it there. While an extension provides, it
//! reads the kernel's handles and what its dependencies already provided
//! through [`StateBuilder::state`].
//!
//! The finished state is immutable: the host owns it and hands a clone (which
//! shares every entry) to every factory through
//! [`ExtensionContext::extension_state`], so a router, a readiness check, a
//! gateway guard factory or a job reads the same instance the extension
//! built, with no global.
//!
//! The authz hook is built before the extension state exists, because the
//! hook is an input to the context the extensions provide from. Its factories
//! receive a [`StateHandle`] instead: the host binds it to the finished state
//! once, before anything serves, and a read before binding fails with
//! [`ExtensionStateError::NotReady`].
//!
//! A type an extension cannot run without is declared in
//! [`Extension::requires`]; the registry checks every enabled extension's
//! declarations against the built state before anything serves, so an
//! unlinked or disabled provider fails boot rather than a request.
//!
//! [`Extension::provide`]: crate::Extension::provide
//! [`Extension::requires`]: crate::Extension::requires
//! [`ExtensionRegistry::build_state`]: crate::ExtensionRegistry::build_state
//! [`ExtensionContext::extension_state`]: crate::ExtensionContext::extension_state
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::sync::{Arc, OnceLock};

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;

use crate::error::ExtensionStateError;

/// The build-phase view of the extension state handed to
/// [`Extension::provide`](crate::Extension::provide), scoped to the one
/// extension currently providing.
pub struct StateBuilder<'a> {
    state: &'a mut Dependencies,
    current: ExtensionId,
}

impl<'a> StateBuilder<'a> {
    pub(crate) const fn new(state: &'a mut Dependencies, current: ExtensionId) -> Self {
        Self { state, current }
    }
}

impl StateBuilder<'_> {
    pub fn provide<T: Any + Send + Sync>(&mut self, value: T) -> Result<(), ExtensionStateError> {
        self.provide_arc(Arc::new(value))
    }

    pub fn provide_arc<T: Any + Send + Sync>(
        &mut self,
        value: Arc<T>,
    ) -> Result<(), ExtensionStateError> {
        Ok(self.state.provide(&self.current, value)?)
    }

    #[must_use]
    pub const fn state(&self) -> &Dependencies {
        self.state
    }

    #[must_use]
    pub const fn extension(&self) -> &ExtensionId {
        &self.current
    }
}

impl std::fmt::Debug for StateBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateBuilder")
            .field("current", &self.current)
            .field("provided", &self.state.len())
            .finish_non_exhaustive()
    }
}

/// A type an extension declares it cannot run without, checked against the
/// built extension state at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateRequirement {
    type_id: TypeId,
    type_name: &'static str,
}

impl StateRequirement {
    #[must_use]
    pub fn of<T: Any + Send + Sync>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    #[must_use]
    pub fn is_met_by(&self, state: &Dependencies) -> bool {
        state.contains_type(self.type_id)
    }
}

/// The extension state for a factory that runs before the state is built.
///
/// The host creates the handle first, hands it to every authz hook factory,
/// and binds it once to the finished state before anything serves. A read
/// before binding is [`ExtensionStateError::NotReady`], never an empty map.
#[derive(Clone, Default)]
pub struct StateHandle(Arc<OnceLock<Dependencies>>);

impl StateHandle {
    pub fn bind(&self, state: Dependencies) -> Result<(), ExtensionStateError> {
        self.0
            .set(state)
            .map_err(|_bound| ExtensionStateError::AlreadyBound)
    }

    pub fn state(&self) -> Result<&Dependencies, ExtensionStateError> {
        self.0.get().ok_or(ExtensionStateError::NotReady)
    }

    pub fn get_arc<T: Any + Send + Sync>(&self) -> Result<Arc<T>, ExtensionStateError> {
        Ok(self.state()?.get_arc::<T>()?)
    }
}

impl std::fmt::Debug for StateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateHandle")
            .field("bound", &self.0.get().is_some())
            .finish()
    }
}

/// A state entry an extension provides empty and the host binds once, after
/// the state is built, to a value composed later (a consumer the host starts,
/// say). Clones share the binding; a read before binding is `None`.
pub struct LateBound<T>(Arc<OnceLock<Arc<T>>>);

impl<T> LateBound<T> {
    pub fn bind(&self, value: Arc<T>) -> Result<(), ExtensionStateError> {
        self.0
            .set(value)
            .map_err(|_bound| ExtensionStateError::AlreadyBound)
    }

    pub fn get_or_init(&self, init: impl FnOnce() -> Arc<T>) -> &Arc<T> {
        self.0.get_or_init(init)
    }

    #[must_use]
    pub fn get(&self) -> Option<&Arc<T>> {
        self.0.get()
    }
}

impl<T> Clone for LateBound<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Default for LateBound<T> {
    fn default() -> Self {
        Self(Arc::new(OnceLock::new()))
    }
}

impl<T> std::fmt::Debug for LateBound<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LateBound")
            .field("bound", &self.0.get().is_some())
            .finish()
    }
}
