//! The proxy as owned by one process: its role, the loopback endpoint every
//! writer is handed, and the hot-swappable runtime config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod serve;

use std::sync::Arc;

use super::bind::portfile_port;
use super::identity::InstallId;
use super::peer::{self, PeerIdentity};
use super::role::{ProxyDeps, ProxyFailure, ProxyRole};
use super::token_cache::{AuthState, TokenCache};
use super::{DEFAULT_PROXY_PORT, LoopbackEndpoint, ServedProxy, portfile};
use systemprompt_identifiers::SessionId;

use crate::config::{self, RuntimeConfig, SharedRuntimeConfig};
use crate::obs::StartupFault;

/// The proxy as owned by one process: its role, the loopback endpoint every
/// writer is handed, and the hot-swappable runtime config.
///
/// Built once by the composition root (the bridge context) and
/// injected; nothing below the context reaches it ambiently.
pub struct ProxyHandle {
    role: ProxyRole,
    loopback: LoopbackEndpoint,
    deps: ProxyDeps,
    runtime_config: SharedRuntimeConfig,
    token_cache: Option<Arc<TokenCache>>,
    session_id: Option<SessionId>,
}

impl std::fmt::Debug for ProxyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyHandle")
            .field("role", &self.role)
            .field("loopback", &self.loopback)
            .finish_non_exhaustive()
    }
}

impl ProxyHandle {
    pub fn attach(deps: ProxyDeps, faults: &mut Vec<StartupFault>) -> Self {
        let port = match portfile_port(&deps.install_id) {
            Ok(port) => port.unwrap_or(DEFAULT_PROXY_PORT),
            Err(e) => {
                faults.push(StartupFault::new("proxy port file", e));
                DEFAULT_PROXY_PORT
            },
        };
        let runtime_config = runtime_config_or_default(faults);
        Self::not_serving(deps, runtime_config, port, ProxyRole::Attached)
    }

    const fn failed(
        deps: ProxyDeps,
        runtime_config: SharedRuntimeConfig,
        failure: ProxyFailure,
    ) -> Self {
        Self::not_serving(
            deps,
            runtime_config,
            DEFAULT_PROXY_PORT,
            ProxyRole::Failed(failure),
        )
    }

    const fn not_serving(
        deps: ProxyDeps,
        runtime_config: SharedRuntimeConfig,
        port: u16,
        role: ProxyRole,
    ) -> Self {
        Self {
            role,
            loopback: LoopbackEndpoint::new(port, None),
            deps,
            runtime_config,
            token_cache: None,
            session_id: None,
        }
    }

    #[must_use]
    pub const fn install_id(&self) -> &InstallId {
        &self.deps.install_id
    }

    #[must_use]
    pub fn peer(&self) -> PeerIdentity {
        peer::probe_identity(self.port(), &self.deps.install_id)
    }

    pub fn forget_recorded_port(&self) -> std::io::Result<()> {
        portfile::clear(&self.deps.install_id)
    }

    #[must_use]
    pub const fn role(&self) -> &ProxyRole {
        &self.role
    }

    #[must_use]
    pub const fn served(&self) -> Option<&ServedProxy> {
        match &self.role {
            ProxyRole::Serving(s) => Some(s),
            ProxyRole::Attached | ProxyRole::AlreadyRunning { .. } | ProxyRole::Failed(_) => None,
        }
    }

    #[must_use]
    pub const fn is_serving(&self) -> bool {
        self.served().is_some()
    }

    #[must_use]
    pub const fn loopback(&self) -> &LoopbackEndpoint {
        &self.loopback
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.loopback.port()
    }

    #[must_use]
    pub const fn runtime_config(&self) -> &SharedRuntimeConfig {
        &self.runtime_config
    }

    pub fn reload_runtime_config(&self) -> Result<(), config::ConfigReadError> {
        self.runtime_config
            .store(Arc::new(RuntimeConfig::from_loaded()?));
        if let Some(cache) = &self.token_cache {
            let cache = Arc::clone(cache);
            if let ProxyRole::Serving(served) = &self.role {
                served.tasks.spawn(async move { cache.reset().await });
            }
        }
        tracing::info!(target: "bridge::config", "runtime config swapped");
        Ok(())
    }

    #[must_use]
    pub const fn token_cache(&self) -> Option<&Arc<TokenCache>> {
        self.token_cache.as_ref()
    }

    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    #[must_use]
    pub fn auth_state(&self) -> Option<tokio::sync::watch::Receiver<AuthState>> {
        self.token_cache.as_ref().map(|cache| cache.auth_state())
    }

    #[must_use]
    pub fn sign_in_required(&self) -> bool {
        self.token_cache
            .as_ref()
            .is_some_and(|cache| cache.sign_in_required())
    }

    pub fn credential_proven(&self) {
        if let Some(cache) = &self.token_cache {
            cache.credential_proven();
        }
    }
}

fn runtime_config_or_default(faults: &mut Vec<StartupFault>) -> SharedRuntimeConfig {
    match config::shared_from_loaded() {
        Ok(cfg) => cfg,
        Err(e) => {
            faults.push(StartupFault::new("config", e));
            config::shared_from_config(&config::Config::default())
        },
    }
}
