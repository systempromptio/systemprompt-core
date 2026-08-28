//! Cancellation tokens for long-running GUI operations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tokio_util::sync::CancellationToken;

use super::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelScope {
    Sync,
    Login,
    // Why: saving the gateway gets its own scope, not `Login`. The gateway
    // field's blur fires `gateway.set` as the sign-in button is clicked, so
    // sharing a scope let each one destroy the other's token — the sign-in
    // that followed became uncancellable and its Cancel button did nothing.
    SetGateway,
    GatewayProbe,
}

#[derive(Debug, Default)]
pub(super) struct CancelTokens {
    sync: Option<CancellationToken>,
    login: Option<CancellationToken>,
    set_gateway: Option<CancellationToken>,
    gateway_probe: Option<CancellationToken>,
}

impl AppState {
    pub fn install_cancel(&self, scope: CancelScope) -> CancellationToken {
        let token = CancellationToken::new();
        let prev = {
            let mut guard = self.cancels.write();
            let prev = match scope {
                CancelScope::Sync => guard.sync.replace(token.clone()),
                CancelScope::Login => guard.login.replace(token.clone()),
                CancelScope::SetGateway => guard.set_gateway.replace(token.clone()),
                CancelScope::GatewayProbe => guard.gateway_probe.replace(token.clone()),
            };
            drop(guard);
            prev
        };
        if let Some(prev) = prev {
            prev.cancel();
        }
        token
    }

    pub fn clear_cancel(&self, scope: CancelScope) {
        let mut guard = self.cancels.write();
        match scope {
            CancelScope::Sync => guard.sync = None,
            CancelScope::Login => guard.login = None,
            CancelScope::SetGateway => guard.set_gateway = None,
            CancelScope::GatewayProbe => guard.gateway_probe = None,
        }
        drop(guard);
    }

    pub fn cancel_scope(&self, scope: CancelScope) -> bool {
        let taken = {
            let mut guard = self.cancels.write();
            match scope {
                CancelScope::Sync => guard.sync.take(),
                CancelScope::Login => guard.login.take(),
                CancelScope::SetGateway => guard.set_gateway.take(),
                CancelScope::GatewayProbe => guard.gateway_probe.take(),
            }
        };
        taken.is_some_and(|token| {
            token.cancel();
            true
        })
    }

    pub fn cancel_all(&self) {
        let mut guard = self.cancels.write();
        for token in [
            guard.sync.take(),
            guard.login.take(),
            guard.gateway_probe.take(),
        ]
        .into_iter()
        .flatten()
        {
            token.cancel();
        }
    }
}
