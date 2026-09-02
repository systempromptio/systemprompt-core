//! Process-wide services-config bootstrap.
//!
//! Mirrors `systemprompt_config::ProfileBootstrap` for the services tree: the
//! merged, validated [`ServicesConfig`] — provider catalog, resolved gateway,
//! agents, MCP servers — is loaded once, right after the profile, and read
//! through `&'static` accessors for the life of the process. A load failure is
//! a boot failure: nothing downstream may run against a catalog that did not
//! parse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::OnceLock;

use systemprompt_models::services::{GatewayConfig, ProviderRegistry, ServicesConfig};

use crate::config_loader::ConfigLoader;
use crate::error::{ConfigLoadError, ConfigLoadResult};

static SERVICES: OnceLock<ServicesConfig> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ServicesBootstrap;

impl ServicesBootstrap {
    pub fn init() -> ConfigLoadResult<&'static ServicesConfig> {
        if SERVICES.get().is_some() {
            return Err(ConfigLoadError::AlreadyInitialized);
        }
        let services = ConfigLoader::load()?;
        Self::install(services)
    }

    pub fn init_from_path(path: &Path) -> ConfigLoadResult<&'static ServicesConfig> {
        if SERVICES.get().is_some() {
            return Err(ConfigLoadError::AlreadyInitialized);
        }
        let services = ConfigLoader::load_from_path(path)?;
        Self::install(services)
    }

    pub fn try_init() -> ConfigLoadResult<&'static ServicesConfig> {
        if let Some(services) = SERVICES.get() {
            return Ok(services);
        }
        Self::init()
    }

    pub fn get() -> ConfigLoadResult<&'static ServicesConfig> {
        SERVICES.get().ok_or(ConfigLoadError::NotInitialized)
    }

    pub fn providers() -> ConfigLoadResult<&'static ProviderRegistry> {
        Self::get().map(|s| &s.providers)
    }

    pub fn gateway() -> ConfigLoadResult<Option<&'static GatewayConfig>> {
        Self::get().map(ServicesConfig::gateway_config)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        SERVICES.get().is_some()
    }

    fn install(services: ServicesConfig) -> ConfigLoadResult<&'static ServicesConfig> {
        SERVICES
            .set(services)
            .map_err(|_already| ConfigLoadError::AlreadyInitialized)?;
        SERVICES.get().ok_or(ConfigLoadError::NotInitialized)
    }
}
