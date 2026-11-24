//! The context's host seeds: the kernel seeds a context requires, the
//! context-built values
//! [`AppContext::from_parts`](super::AppContext::from_parts) seals into the
//! state, and the two host operations computed over the state rather than held
//! in it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CoordinationRepositories, DbPool, ServiceRepository};
use systemprompt_events::{EventBridgeHandle, EventRouter};
use systemprompt_extension::Admitted;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::{GovernanceEngine, PolicyCatalog};
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_models::{BuildIdentity, RouteClassifier};
use systemprompt_provider_contracts::{ConfigPlaneResult, ConfigPlaneSet, Dependencies};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, ConfigWatch, FileStorage};

use super::{ContextPlanes, ShutdownRequest};
use crate::builder::kernel_binders::PendingBinders;
use crate::error::RuntimeResult;
use crate::kernel::Kernel;
use crate::schema_currency::SchemaState;

pub(crate) fn require_kernel_seeds(state: &Dependencies) -> RuntimeResult<()> {
    state.get::<Arc<Config>>()?;
    state.get::<DbPool>()?;
    state.get::<Arc<AppPaths>>()?;
    state.get::<SystemAdmin>()?;
    state.get::<CacheEpochs>()?;
    state.get::<Arc<dyn FileStorage>>()?;
    state.get::<BackgroundTasks>()?;
    state.get::<Arc<EventRouter>>()?;
    Ok(())
}

pub(crate) fn reserve_context_seeds(state: &mut Dependencies) {
    state.reserve::<Arc<ServiceRepository>>();
    state.reserve::<Arc<CoordinationRepositories>>();
    state.reserve::<BootContext>();
    state.reserve::<Arc<RouteClassifier>>();
    state.reserve::<BuildIdentity>();
    state.reserve::<LoggingRuntime>();
    state.reserve::<SharedAuthzHook>();
    state.reserve::<ConfigWatch<GovernanceEngine>>();
    state.reserve::<ServicesState>();
    state.reserve::<Arc<SchemaState>>();
    state.reserve::<Arc<EventBridgeHandle>>();
    state.reserve::<ShutdownRequest>();
    state.reserve::<PendingBinders>();
}

pub(crate) fn seal_context_seeds(kernel: &mut Kernel, planes: ContextPlanes) {
    let ContextPlanes {
        data,
        cfg,
        subsystems,
    } = planes;
    let state = kernel.state_mut();
    state.insert_sealed(data.service_repository);
    state.insert_sealed(data.coordination);
    state.insert_sealed(cfg.boot);
    state.insert_sealed(cfg.route_classifier);
    state.insert_sealed(cfg.build);
    state.insert_sealed(subsystems.logging);
    state.insert_sealed(subsystems.authz_hook);
    state.insert_sealed(subsystems.governance);
    state.insert_sealed(subsystems.services);
    state.insert_sealed(subsystems.schema_install);
    state.insert_sealed(subsystems.event_bridge);
    state.insert_sealed(subsystems.shutdown);
    state.insert_sealed(subsystems.late_binders);
}

/// The config planes every admitted member declares, over the profile's
/// plane authority and services path.
pub fn config_plane_set(admitted: &Admitted, config: &Config) -> ConfigPlaneResult<ConfigPlaneSet> {
    let planes = admitted
        .built()
        .members()
        .flat_map(|member| member.sink.config_planes().iter().cloned())
        .collect();
    ConfigPlaneSet::new(
        planes,
        config.planes.authority.clone(),
        std::path::PathBuf::from(&config.services_path),
    )
}

/// Loads a fresh policy engine from the database over the kernel's
/// policy catalog, for a reload to swap into the governance watch.
pub async fn reload_governance(kernel: &Kernel) -> RuntimeResult<GovernanceEngine> {
    let state = kernel.state();
    let catalog = kernel.resolved().get::<Arc<PolicyCatalog>>()?;
    let boot = state.get::<BootContext>()?;
    let database = state.get::<DbPool>()?;
    Ok(crate::builder::load_governance(boot.profile(), database, &catalog).await?)
}
