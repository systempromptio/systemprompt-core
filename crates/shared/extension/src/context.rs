//! Runtime context handed to extensions during router resolution.
//!
//! Beyond config, database and storage, the context carries the
//! cross-replica seams core owns: [`ExtensionContext::instances`] answers
//! whether a replica is alive, [`ExtensionContext::leases`] hands out
//! expiring single-owner claims, and [`ExtensionContext::advisory_locks`]
//! takes session-scoped locks on the session pool. A context without a
//! database refuses all three through [`UnavailableCoordination`].
//!
//! [`ExtensionContext::secret`] reads an operator-provisioned custom secret
//! from the platform secrets document, so an extension never reads its
//! credentials from the process environment; [`ExtensionContext::instance_id`]
//! is the node identity in place of `HOSTNAME`.
//!
//! [`ExtensionContext::background_tasks`] hands out the process's one
//! [`BackgroundTasks`] owner (a clone shares the tracker), so work an
//! extension starts after a response is drained by the server's shutdown
//! instead of being detached through `tokio::spawn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;
use systemprompt_provider_contracts::{ConfigPlaneError, ConfigPlaneResult, ConfigPlaneSet};
use systemprompt_traits::{
    BackgroundTasks, ConfigProvider, ConfigWatch, DatabaseHandle, DynAdvisoryLocks,
    DynInstanceRegistry, DynLeaseRepository, ExtensionSecret, FileStorage, UnavailableCoordination,
};

pub trait ExtensionContext: Send + Sync {
    fn system_owner_id(&self) -> systemprompt_identifiers::UserId;

    fn config(&self) -> Arc<dyn ConfigProvider>;

    fn database(&self) -> Arc<dyn DatabaseHandle>;

    fn file_storage(&self) -> Arc<dyn FileStorage>;

    fn instance_id(&self) -> InstanceId;

    fn background_tasks(&self) -> BackgroundTasks;

    fn instances(&self) -> DynInstanceRegistry {
        Arc::new(UnavailableCoordination)
    }

    fn leases(&self) -> DynLeaseRepository {
        Arc::new(UnavailableCoordination)
    }

    fn advisory_locks(&self) -> DynAdvisoryLocks {
        Arc::new(UnavailableCoordination)
    }

    fn secret(&self, _name: &str) -> Option<ExtensionSecret> {
        None
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }

    fn services_config(&self) -> ConfigWatch<ServicesConfig>;

    fn resolved_catalog(&self) -> Arc<ResolvedCatalog>;

    fn config_planes(&self) -> ConfigPlaneResult<ConfigPlaneSet> {
        Err(ConfigPlaneError::PlanesUnavailable)
    }
}

pub type DynExtensionContext = Arc<dyn ExtensionContext>;
