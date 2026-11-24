//! The handles a host seeds its kernel state from: [`KernelSeed`] for every
//! host, [`ApiKernelSeed`] for what only the API adds, and the coordination
//! seams ([`KernelCoordination`]) both carry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::DynChildSecretEnv;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::{ActiveProfilePath, ActiveServicesRoot};
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::ContentConfigRaw;
use systemprompt_models::authz::CatalogRoleNames;

use systemprompt_models::auth::TokenAuthority;
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::{SigningKeys, VerificationKeyring};
use systemprompt_traits::{
    AtRestPepper, BackgroundTasks, CacheEpochs, DynAdvisoryLocks, DynInstanceRegistry,
    DynLeaseRepository, EscalationDelivery, FileStorage, SecretLookup, SecretStore,
};

/// The cross-replica coordination seams every host seeds.
pub struct KernelCoordination {
    pub instances: DynInstanceRegistry,
    pub leases: DynLeaseRepository,
    pub locks: DynAdvisoryLocks,
}

/// The handles every host seeds its kernel state from.
pub struct KernelSeed<'a> {
    pub database: &'a DbPool,
    pub app_paths: &'a Arc<AppPaths>,
    pub files_config: &'a Arc<FilesConfig>,
    pub profile: &'a Profile,
    pub config: &'a Arc<Config>,
    pub cache_epochs: &'a CacheEpochs,
    pub verification: &'a VerificationKeyring,
    pub services: systemprompt_manifest::services::ServicesView,
    pub services_root: ActiveServicesRoot,
    pub profile_path: ActiveProfilePath,
    pub secrets: Arc<dyn SecretStore>,
    pub secret_lookup: Arc<dyn SecretLookup>,
    pub connector_tokens: Arc<TokenCache>,
    pub file_storage: Arc<dyn FileStorage>,
    pub background_tasks: BackgroundTasks,
    pub coordination: KernelCoordination,
    pub system_admin: &'a SystemAdmin,
    pub event_router: &'a EventRouter,
    pub documents: DocumentsHandlesSlot,
    pub catalog_roles: CatalogRoleNames,
    pub escalation: EscalationDelivery,
}

/// The handles only the API seeds on top of [`KernelSeed`].
pub struct ApiKernelSeed<'a> {
    pub base: KernelSeed<'a>,
    pub signing_keys: &'a SigningKeys,
    pub token_authority: &'a Arc<dyn TokenAuthority>,
    pub spawner: &'a Arc<Spawner>,
    pub child_secrets: DynChildSecretEnv,
    pub at_rest_pepper: AtRestPepper,
    pub content_config: Option<&'a Arc<ContentConfigRaw>>,
}

impl KernelCoordination {
    #[must_use]
    pub fn over(
        services: &Arc<systemprompt_database::ServiceRepository>,
        coordination: &systemprompt_database::CoordinationRepositories,
    ) -> Self {
        Self {
            instances: Arc::<systemprompt_database::ServiceRepository>::clone(services),
            leases: Arc::<systemprompt_database::InstanceLeaseRepository>::clone(
                &coordination.leases,
            ),
            locks: Arc::<systemprompt_database::SessionAdvisoryLocks>::clone(&coordination.locks),
        }
    }
}

impl std::fmt::Debug for KernelCoordination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelCoordination").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for KernelSeed<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelSeed")
            .field("profile", &self.profile.name)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ApiKernelSeed<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKernelSeed")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
