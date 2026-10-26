//! Startup of a serving proxy: config, loopback secret, port bind, server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use tokio::runtime::Handle;

use super::ProxyHandle;
use crate::config;
use crate::config::SharedRuntimeConfig;
use crate::obs::StartupFault;
use crate::proxy::bind::{Bind, bind_candidate, persist_and_announce};
use crate::proxy::refresh::refresh_loop;
use crate::proxy::role::{ProxyDeps, ProxyFailure, ProxyRole};
use crate::proxy::session::SessionContext;
use crate::proxy::token_cache::TokenCache;
use crate::proxy::{LoopbackEndpoint, secret, server};

struct Bound {
    loopback_secret: crate::ids::LoopbackSecret,
    listener: tokio::net::TcpListener,
}

impl ProxyHandle {
    pub fn serve(rt: &Handle, deps: ProxyDeps, faults: &mut Vec<StartupFault>) -> Self {
        // Why: a proxy that cannot read its config would serve inference
        // against the brand default gateway with whatever credential is on
        // disk; refusing to bind is the only safe answer.
        let runtime_config = match config::shared_from_loaded() {
            Ok(cfg) => cfg,
            Err(e) => {
                faults.push(StartupFault::new("config", &e));
                return Self::failed(
                    deps,
                    config::shared_from_config(&config::Config::default()),
                    ProxyFailure::Config(e),
                );
            },
        };
        let mut tried = Vec::new();
        let mut last_error = "no candidate port could be bound".to_owned();

        // Why: the secret is what makes a bound port usable. Reading it first
        // keeps a port from being taken and then abandoned, and reports the
        // real fault by name instead of an empty list of tried ports.
        let loopback_secret = match secret::proxy_init() {
            Ok(s) => s,
            Err(e) => {
                faults.push(StartupFault::new("loopback secret", &e));
                return Self::failed(deps, runtime_config, ProxyFailure::LoopbackSecret(e));
            },
        };

        let listener =
            match bind_candidate(rt, &deps.install_id, &mut tried, &mut last_error, faults) {
                Bind::Listener(l) => l,
                Bind::Sibling {
                    port,
                    pid,
                    config_dir,
                } => {
                    return Self::not_serving(
                        deps,
                        runtime_config,
                        port,
                        ProxyRole::AlreadyRunning {
                            port,
                            pid,
                            config_dir,
                        },
                    );
                },
                Bind::Exhausted => {
                    return Self::failed(
                        deps,
                        runtime_config,
                        ProxyFailure::Bind { tried, last_error },
                    );
                },
            };

        Self::start(
            rt,
            deps,
            runtime_config,
            Bound {
                loopback_secret,
                listener,
            },
            faults,
        )
    }

    fn start(
        rt: &Handle,
        deps: ProxyDeps,
        runtime_config: SharedRuntimeConfig,
        bound: Bound,
        faults: &mut Vec<StartupFault>,
    ) -> Self {
        let Bound {
            loopback_secret,
            listener,
        } = bound;
        let session_context = Arc::new(SessionContext::new());
        let session_id = session_context.session_id().clone();
        let token_cache = Arc::new(TokenCache::default_for_runtime(
            session_context.session_id().clone(),
            deps.http.clone(),
        ));
        let parts = server::ServerParts {
            loopback: loopback_secret.clone(),
            runtime_config: Arc::clone(&runtime_config),
            token_cache: Arc::clone(&token_cache),
            session: session_context,
            deps: deps.clone(),
        };
        let served = match server::start_with_listener(rt, listener, parts) {
            Ok(s) => s,
            Err(e) => return Self::failed(deps, runtime_config, ProxyFailure::Server(e)),
        };

        // Why: the listener is already serving. An unwritable port file
        // means the other processes cannot find it, which `doctor` reports;
        // it does not make the bound port any less real.
        if let Err(e) = persist_and_announce(served.port, &deps.install_id) {
            tracing::error!(port = served.port, error = %e, "proxy port could not be published");
            faults.push(StartupFault::new("proxy port file", e));
        }
        served.tasks.spawn(refresh_loop(Arc::clone(&token_cache)));

        Self {
            loopback: LoopbackEndpoint::new(served.port, Some(loopback_secret)),
            role: ProxyRole::Serving(served),
            deps,
            runtime_config,
            token_cache: Some(token_cache),
            session_id: Some(session_id),
        }
    }
}
