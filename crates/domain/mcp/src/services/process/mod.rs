//! OS-process lifecycle for MCP servers: spawning, liveness, port holders
//! and stops.
//!
//! Spawning is local ([`spawner`]); everything that probes or signals a live
//! process goes through [`systemprompt_loader::subprocess`], so an MCP server
//! is stopped only when it is provably this installation's child for that
//! service (its recorded pid carries the matching [`ChildKind::Mcp`] marker
//! and the boot nonce the row recorded, [`SpawnStamp`]). A stop waits the
//! profile's `server.shutdown.child_grace_secs`, passed in by the caller from
//! the configuration it holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod child_env;
pub mod spawner;

use std::sync::Arc;
use std::time::Duration;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use systemprompt_config::DynChildSecretEnv;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::{ServiceName, SpawnNonce};
use systemprompt_loader::subprocess::{
    self, ChildClaim, ChildKind, SpawnStamp, Spawner, StopOutcome,
};
use systemprompt_manifest::Config;
use systemprompt_security::keys::TokenVerificationKeys;

#[derive(Debug, Clone)]
pub struct ProcessService {
    spawner: Arc<Spawner>,
    config: Arc<Config>,
    secrets: DynChildSecretEnv,
    verification_keys: TokenVerificationKeys,
}

impl ProcessService {
    pub const fn new(
        spawner: Arc<Spawner>,
        config: Arc<Config>,
        secrets: DynChildSecretEnv,
        verification_keys: TokenVerificationKeys,
    ) -> Self {
        Self {
            spawner,
            config,
            secrets,
            verification_keys,
        }
    }

    pub fn spawn_server(&self, paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<u32> {
        let secrets = spawner::SpawnSecrets {
            env: self.secrets.as_ref(),
            verification_keys: &self.verification_keys,
        };
        spawner::spawn_server(&self.spawner, &self.config, &secrets, paths, config)
    }

    #[must_use]
    pub fn nonce(&self) -> &SpawnNonce {
        self.spawner.nonce()
    }

    #[must_use]
    pub fn child_grace(&self) -> Duration {
        self.config.shutdown.child_grace()
    }

    pub async fn is_running(pid: u32) -> bool {
        subprocess::is_running(pid).await
    }

    #[must_use]
    pub fn claim(service_name: &ServiceName, stamp: &SpawnStamp) -> ChildClaim {
        ChildClaim::spawned(ChildKind::Mcp, service_name, stamp.clone())
    }

    pub async fn owns(pid: u32, service_name: &ServiceName, stamp: &SpawnStamp) -> bool {
        subprocess::owns(pid, &Self::claim(service_name, stamp)).await
    }

    pub async fn owned_port_holders(
        port: u16,
        service_name: &ServiceName,
        stamp: &SpawnStamp,
    ) -> McpDomainResult<Vec<u32>> {
        let claim = Self::claim(service_name, stamp);
        let mut owned = Vec::new();
        for pid in subprocess::pids_listening_on(port).await? {
            if subprocess::owns(pid, &claim).await {
                owned.push(pid);
            }
        }
        Ok(owned)
    }

    pub async fn port_holders(port: u16) -> McpDomainResult<Vec<u32>> {
        Ok(subprocess::pids_listening_on(port).await?)
    }

    pub async fn port_has_listener(port: u16) -> McpDomainResult<bool> {
        Ok(!Self::port_holders(port).await?.is_empty())
    }

    pub fn verify_binary(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<()> {
        spawner::verify_binary(paths, config)
    }

    pub async fn build_server(config: &McpServerConfig) -> McpDomainResult<()> {
        let config = config.clone();
        tokio::task::spawn_blocking(move || spawner::build_server(&config)).await?
    }

    pub async fn stop(
        pid: u32,
        service_name: &ServiceName,
        stamp: &SpawnStamp,
        grace: Duration,
    ) -> McpDomainResult<StopOutcome> {
        Ok(subprocess::stop_owned(pid, &Self::claim(service_name, stamp), grace).await?)
    }
}
