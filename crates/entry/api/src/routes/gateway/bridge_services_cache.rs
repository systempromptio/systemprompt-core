//! A short-lived in-process copy of the services config for the bridge routes.
//!
//! One bridge sync is a manifest fetch followed by a download per plugin
//! file, and each of those used to re-read and re-parse every services YAML
//! file. The copy here is reused for `TTL`, so a sync burst parses the tree
//! once while an edit on disk is still picked up within seconds. Per-user
//! filtering happens downstream; the config itself is the same for everyone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_loader::ConfigLoadResult;
use systemprompt_manifest::services::ServicesConfig;

use super::bridge_data;

const TTL: Duration = Duration::from_secs(10);

static CACHED: Mutex<Option<(Instant, ServicesConfig)>> = Mutex::new(None);

pub fn load_services_config() -> ConfigLoadResult<ServicesConfig> {
    if let Some((loaded_at, config)) = CACHED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        && loaded_at.elapsed() < TTL
    {
        return Ok(config.clone());
    }
    let config = bridge_data::load_services_config()?;
    *CACHED.lock().unwrap_or_else(PoisonError::into_inner) = Some((Instant::now(), config.clone()));
    Ok(config)
}
