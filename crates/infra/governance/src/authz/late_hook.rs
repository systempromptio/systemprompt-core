//! The deployment's authz hook, read before the host has built it.
//!
//! A host builds its one authz hook after its kernel resolves, from the
//! subject providers and hooks the resolve built. A member that assembles a
//! service in its provide holds [`LateAuthzHook`] instead: it reads the
//! sealed [`AuthzHookCell`] the host seeds and decides through whatever
//! hook the host binds into it. Until the host binds the cell every request
//! is denied, so a call that runs too early never sees a default allow. The
//! write half, [`AuthzHookBinder`], stays with the host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_models::{LateBinder, LateRead};

use super::hook::{AuthzDecisionHook, DenyAllHook};
use super::types::{AuthzDecision, AuthzRequest};

pub type AuthzHookCell = LateRead<dyn AuthzDecisionHook>;

pub type AuthzHookBinder = LateBinder<dyn AuthzDecisionHook>;

/// The hook a host binds into its [`AuthzHookCell`]; it denies every request
/// until the host has bound one.
#[derive(Debug, Clone)]
pub struct LateAuthzHook {
    cell: AuthzHookCell,
}

impl LateAuthzHook {
    #[must_use]
    pub const fn new(cell: AuthzHookCell) -> Self {
        Self { cell }
    }
}

#[async_trait]
impl AuthzDecisionHook for LateAuthzHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        match self.cell.get() {
            Some(hook) => hook.evaluate(req).await,
            None => DenyAllHook::null().evaluate(req).await,
        }
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        match self.cell.get() {
            Some(hook) => hook.probe(req).await,
            None => DenyAllHook::null().probe(req).await,
        }
    }
}
