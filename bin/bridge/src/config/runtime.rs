//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::config::{self, Config};

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub gateway_base: Arc<ValidatedUrl>,
}

impl RuntimeConfig {
    #[must_use]
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            gateway_base: Arc::new(config::gateway_url_or_default(cfg)),
        }
    }

    #[must_use]
    pub fn from_loaded() -> Self {
        Self::from_config(&config::load())
    }
}

pub type SharedRuntimeConfig = Arc<ArcSwap<RuntimeConfig>>;

#[must_use]
pub fn shared_from_loaded() -> SharedRuntimeConfig {
    Arc::new(ArcSwap::from_pointee(RuntimeConfig::from_loaded()))
}
