//! Runtime context handed to extensions during router resolution.
//!
//! Beyond config, database and storage, the context carries the
//! cross-replica seams core owns: [`ExtensionContext::instances`] answers
//! whether a replica is alive, [`ExtensionContext::leases`] hands out
//! expiring single-owner claims, and [`ExtensionContext::advisory_locks`]
//! takes session-scoped locks on the session pool. A context without a
//! database refuses all three through [`UnavailableCoordination`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::InstanceId;
use systemprompt_provider_contracts::{ConfigPlaneError, ConfigPlaneResult, ConfigPlaneSet};
use systemprompt_traits::{
    ConfigProvider, DatabaseHandle, DynAdvisoryLocks, DynInstanceRegistry, DynLeaseRepository,
    FileStorage, UnavailableCoordination,
};

pub trait ExtensionContext: Send + Sync {
    fn system_owner_id(&self) -> systemprompt_identifiers::UserId;

    fn config(&self) -> Arc<dyn ConfigProvider>;

    fn database(&self) -> Arc<dyn DatabaseHandle>;

    fn file_storage(&self) -> Arc<dyn FileStorage>;

    fn instance_id(&self) -> InstanceId;

    fn instances(&self) -> DynInstanceRegistry {
        Arc::new(UnavailableCoordination)
    }

    fn leases(&self) -> DynLeaseRepository {
        Arc::new(UnavailableCoordination)
    }

    fn advisory_locks(&self) -> DynAdvisoryLocks {
        Arc::new(UnavailableCoordination)
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }

    fn config_planes(&self) -> ConfigPlaneResult<ConfigPlaneSet> {
        Err(ConfigPlaneError::PlanesUnavailable)
    }
}

pub type DynExtensionContext = Arc<dyn ExtensionContext>;
