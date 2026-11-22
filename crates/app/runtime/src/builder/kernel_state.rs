//! The kernel's own entries in the extension state: the handles the host
//! seeds the state with before any extension provides. [`base_kernel_state`]
//! holds the entries every process seeds, the API and a child host alike,
//! from the handles in a [`KernelSeed`]; [`kernel_state`] adds what only the
//! API composes. An extension reads them from the same map as every other
//! extension's values, and cannot provide a type the kernel already holds.
//!
//! Every host seeds the database handle, the paths, the files, notifications
//! and brain profiles, the instance id, the file storage
//! (`Arc<dyn FileStorage>`), the background-task owner (`BackgroundTasks`),
//! the coordination seams (`DynInstanceRegistry`, `DynLeaseRepository`,
//! `DynAdvisoryLocks`; a child host seeds `UnavailableCoordination` for all
//! three), the runtime `Config`, the services
//! view and active services root, the secret store, the system admin, the
//! event router and the escalation delivery mode the notifications member
//! assembles its service with (`InProcess` on the API, `Consumer` on a
//! child host, which delivers through the serving host's consumer). The
//! document store is built from the resolved state, so it is not seeded;
//! the read half of its cell (`DocumentsHandlesSlot`) is, and the host keeps
//! the bind half outside the state and binds it once the store is built.
//!
//! The runtime `Config`, the secret store, the system admin, the active
//! services root, the profile's services bundle sources (`BundleSources`:
//! the pinned sources, bundle cache and base tree, never another profile
//! value) and the event router are sealed, and so are the JWT signing
//! keys the API seeds: a provide fn, a factory, a slot build, a kernel command
//! and a route set read them, and every context handed to member runtime code
//! (a job, a runner service, a config plane, a member command) is built
//! without them. `Config`, the secret store and the signing keys carry
//! credentials; the system admin, the services root and the bundle sources
//! are boot-only identity and layout; the event router publishes to and
//! subscribes on every user's stream. Every other seed is open. A member whose
//! runtime code needs a capability derived from a sealed seed builds a narrow
//! handle in its provide fn or factory.
//!
//! The process's one `AiService` (present only when inference is configured)
//! is composed from several domains, so the composition root seeds it; an
//! route set or factory resolves them from the built state and
//! an absent `AiService` is a typed `MissingDependency`, never a fallback
//! service. An absent `AiService` is still reserved for the kernel, so no
//! extension can `provide` its own.
//!
//! The messaging dispatcher slot (`LateBound<dyn MessagingDispatcher>`) is
//! seeded unbound: the dispatcher needs the API's agent proxy, which exists
//! only after this state is built, so the API binds it while it builds its
//! routes and the chat-platform surfaces read the slot at dispatch time. The
//! agent dispatch cell (`AgentDispatchCell`) is seeded unbound and **sealed**
//! the same way: the API binds its agent proxy into it, and only kernel code
//! and core provide fns and slot builds read it.
//!
//! The API also seeds the read-side handles a capability's routes are built
//! from: the loaded content configuration (`Arc<ContentConfigRaw>`, whose
//! `ContentRouting` resolves a page URL to a slug) and the configured
//! `GeoIP` database (`GeoIpReader`). Both are optional: when either is not
//! configured its type is reserved, so a reader's `find` is `None` (`get` is
//! a typed `MissingDependency`) and no extension can `provide` a substitute.
//! The analytics repositories are not seeded: the analytics member provides
//! them.
//!
//! No event broadcaster is seeded: a broadcaster's holder can subscribe to,
//! publish on and enumerate every user's stream, so the API hands the
//! analytics broadcaster to the one core route declaration that streams it,
//! never to the shared state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::AiService;
use systemprompt_ai::repository::AiRepositories;
use systemprompt_analytics::GeoIpReader;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::{ContentConfigRaw, LateBound};
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::EscalationDelivery;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    BackgroundTasks, DynAdvisoryLocks, DynInstanceRegistry, DynLeaseRepository, FileStorage,
    MessagingDispatcher, SecretStore,
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
    pub services: systemprompt_manifest::services::ServicesView,
    pub services_root: ActiveServicesRoot,
    pub secrets: Arc<dyn SecretStore>,
    pub file_storage: Arc<dyn FileStorage>,
    pub background_tasks: BackgroundTasks,
    pub coordination: KernelCoordination,
    pub system_admin: &'a SystemAdmin,
    pub event_router: &'a EventRouter,
    pub documents: DocumentsHandlesSlot,
    #[cfg(feature = "notifications")]
    pub escalation: EscalationDelivery,
}

/// The handles only the API seeds on top of [`KernelSeed`].
pub struct ApiKernelSeed<'a> {
    pub base: KernelSeed<'a>,
    pub signing_keys: &'a SigningKeys,
    pub oauth: &'a Arc<OAuthRepositories>,
    pub ai: &'a Arc<AiRepositories>,
    pub ai_service: Option<&'a Arc<AiService>>,
    pub geoip: Option<&'a GeoIpReader>,
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
            .field("ai_service", &self.ai_service.is_some())
            .finish_non_exhaustive()
    }
}

pub fn base_kernel_state(seed: KernelSeed<'_>) -> Dependencies {
    let state = Dependencies::new()
        .with(Arc::clone(seed.database))
        .with(Arc::clone(seed.app_paths))
        .with(Arc::clone(seed.files_config))
        .with(Arc::new(seed.profile.notifications))
        .with(Arc::new(seed.profile.brain.clone()))
        .with(seed.config.instance_id.clone())
        .with(seed.file_storage)
        .with(seed.background_tasks)
        .with(seed.coordination.instances)
        .with(seed.coordination.leases)
        .with(seed.coordination.locks)
        .with_sealed(Arc::clone(seed.config))
        .with(seed.services)
        .with_sealed(seed.services_root)
        .with_sealed(BundleSources::of(seed.profile))
        .with_sealed(seed.secrets)
        .with_sealed(seed.system_admin.clone())
        .with_sealed(Arc::new(seed.event_router.clone()))
        .with(seed.documents);
    #[cfg(feature = "notifications")]
    let state = state.with(seed.escalation);
    state
}

pub fn kernel_state(seed: ApiKernelSeed<'_>) -> Dependencies {
    let mut state = base_kernel_state(seed.base)
        .with_sealed(seed.signing_keys.clone())
        .with(Arc::clone(seed.oauth))
        .with(Arc::clone(seed.ai))
        .with(LateBound::<dyn MessagingDispatcher>::default())
        .with_sealed(AgentDispatchCell::default());
    match seed.ai_service {
        Some(ai) => state.insert(Arc::clone(ai)),
        None => state.reserve::<Arc<AiService>>(),
    }
    match seed.geoip {
        Some(reader) => state.insert(Arc::clone(reader)),
        None => state.reserve::<GeoIpReader>(),
    }
    match seed.content_config {
        Some(config) => state.insert(Arc::clone(config)),
        None => state.reserve::<Arc<ContentConfigRaw>>(),
    }
    state
}
