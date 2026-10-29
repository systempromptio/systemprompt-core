//! Runtime configuration snapshot assembled at process start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::config::{self, Config};

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub gateway_base: Arc<ValidatedUrl>,
    pub disabled: bool,
    pub foreign_models: Arc<std::collections::BTreeMap<String, super::gateways::GatewayProfileId>>,
}

impl RuntimeConfig {
    #[must_use]
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            gateway_base: Arc::new(config::gateway_url_or_default(cfg)),
            disabled: cfg.is_disabled(),
            foreign_models: Arc::new(foreign_models()),
        }
    }

    pub fn from_loaded() -> Result<Self, config::ConfigReadError> {
        Ok(Self::from_config(&config::load()?))
    }
}

fn foreign_models() -> std::collections::BTreeMap<String, super::gateways::GatewayProfileId> {
    let resolved = super::gateways::active().and_then(|active| {
        Ok(super::gateways::models_served_elsewhere(
            &active.id,
            &super::gateways::served_models_by_profile()?,
        ))
    });
    resolved.unwrap_or_else(|error| {
        tracing::warn!(%error, "gateway profile models unreadable; no model is refused as another gateway's");
        std::collections::BTreeMap::new()
    })
}

pub type SharedRuntimeConfig = Arc<ArcSwap<RuntimeConfig>>;

pub fn shared_from_loaded() -> Result<SharedRuntimeConfig, config::ConfigReadError> {
    Ok(shared_from_config(&config::load()?))
}

#[must_use]
pub fn shared_from_config(cfg: &Config) -> SharedRuntimeConfig {
    Arc::new(ArcSwap::from_pointee(RuntimeConfig::from_config(cfg)))
}
