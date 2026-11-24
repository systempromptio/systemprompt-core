//! Typed, extension-scoped state built once at boot.
//!
//! The state is a [`Dependencies`] map — the one type-keyed map the platform
//! hands to every factory, job, config plane and prerender context. The
//! runtime seeds it with the kernel's own handles, then runs every admitted
//! member's provide fns (`Contributions::provide`) in composition order; a
//! second provider of a type already present is refused, whether the kernel
//! or another member put it there. A member reads it through `Resolved`.
//!
//! A type a member cannot run without is declared with
//! `Contributions::requires`, as a
//! [`StateRequirement`](systemprompt_provider_contracts::StateRequirement)
//! checked against the
//! built state before anything serves, so an unlinked or disabled provider
//! fails boot rather than a request.
//!
//! The authz hook is built before the extension state exists, because the
//! hook is an input to the context the members provide from. Its factories
//! receive a [`StateHandle`] instead: the host binds it to the finished state
//! once, before anything serves, and a read before binding fails with
//! [`ExtensionStateError::NotReady`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::sync::Arc;

use systemprompt_models::LateBound;
use systemprompt_provider_contracts::Dependencies;

use crate::error::ExtensionStateError;

/// The extension state for a factory that runs before the state is built.
///
/// The host creates the handle first, hands it to every authz hook factory,
/// and binds it once to the finished state before anything serves. A read
/// before binding is [`ExtensionStateError::NotReady`], never an empty map.
#[derive(Clone, Default)]
pub struct StateHandle(LateBound<Dependencies>);

impl StateHandle {
    pub fn bind(&self, state: Dependencies) -> Result<(), ExtensionStateError> {
        self.0
            .bind(Arc::new(state))
            .map_err(|_bound| ExtensionStateError::AlreadyBound)
    }

    pub fn state(&self) -> Result<&Dependencies, ExtensionStateError> {
        self.0
            .get()
            .map(|state| &**state)
            .ok_or(ExtensionStateError::NotReady)
    }

    pub fn get_arc<T: Any + Send + Sync>(&self) -> Result<Arc<T>, ExtensionStateError> {
        Ok(self.state()?.get_arc::<T>()?)
    }
}

impl std::fmt::Debug for StateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateHandle")
            .field("bound", &self.0.is_bound())
            .finish()
    }
}
