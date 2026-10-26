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
//! [`ExtensionContext::profile`], [`ExtensionContext::app_config`] and
//! [`ExtensionContext::services_root`] hand an extension the profile it was
//! booted from, the resolved runtime configuration and the active services
//! directory, and [`ExtensionContext::token_authority`] the deployment's
//! [`TokenAuthority`] for verifying and minting its own JWTs. An extension
//! never reads the signing keys, the process environment or a global for any
//! of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_provider_contracts::{ConfigPlaneError, ConfigPlaneResult, ConfigPlaneSet};
use systemprompt_traits::{
    BackgroundTasks, ConfigProvider, DatabaseHandle, DynAdvisoryLocks, DynInstanceRegistry,
    DynLeaseRepository, FileStorage, ProviderTokenCache, SecretStore, UnavailableCoordination,
};

pub trait ExtensionContext: Send + Sync {
    fn system_owner_id(&self) -> systemprompt_identifiers::UserId;

    fn config(&self) -> Arc<dyn ConfigProvider>;

    fn profile(&self) -> Arc<Profile>;

    fn app_config(&self) -> Arc<Config>;

    fn services_root(&self) -> systemprompt_manifest::services::root::ActiveServicesRoot;

    fn token_authority(&self) -> Arc<dyn TokenAuthority>;

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

    fn secrets(&self) -> Arc<dyn SecretStore>;

    fn get_extension(&self, id: &str) -> Option<Arc<dyn crate::Extension>>;

    fn has_extension(&self, id: &str) -> bool {
        self.get_extension(id).is_some()
    }

    fn services(&self) -> ServicesView;

    fn subject_attributes(&self) -> Arc<dyn SubjectAttributeSource>;

    fn provider_tokens(&self) -> Arc<dyn ProviderTokenCache>;

    fn config_planes(&self) -> ConfigPlaneResult<ConfigPlaneSet> {
        Err(ConfigPlaneError::PlanesUnavailable)
    }
}

pub type DynExtensionContext = Arc<dyn ExtensionContext>;
