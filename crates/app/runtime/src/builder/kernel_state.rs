//! The kernel's own entries in the extension state: the handles the host
//! seeds the state with before any extension provides. [`base_kernel_state`]
//! holds the entries every process seeds, the API and a child host alike,
//! from the handles in a [`KernelSeed`]; [`kernel_state`] adds what only the
//! API composes. An extension reads them from the same map as every other
//! extension's values, and cannot provide a type the kernel already holds.
//!
//! Every host seeds the database handle, the paths, the files, notifications
//! and brain profiles, the instance id, the runtime `Config`, the services
//! view and active services root, the secret store, the system admin, the
//! event router and the escalation delivery mode the notifications member
//! assembles its service with (`InProcess` on the API, `Consumer` on a
//! child host, which delivers through the serving host's consumer). The
//! document store is built from the resolved state, so it is not seeded;
//! the read half of its cell (`DocumentsHandlesSlot`) is, and the host keeps
//! the bind half outside the state and binds it once the store is built.
//!
//! The runtime `Config`, the secret store, the system admin, the active
//! services root and the event router are sealed: a provide fn, a factory, a
//! slot build, a kernel command and a route set read them, and the resolver
//! a member's command runs against refuses them as missing. `Config` and the
//! secret store carry credentials; the system admin and the services root are
//! boot-only identity and layout; the event router publishes to and
//! subscribes on every user's stream. Every other seed is open.
//!
//! The process's one `AiService` (present only when inference is configured)
//! is composed from several domains, so the composition root seeds it; an extension router reads them from
//! [`ExtensionContext::extension_state`] and an absent `AiService` is a typed
//! `MissingDependency`, never a fallback service. An absent `AiService` is
//! still reserved for the kernel, so no extension can `provide` its own.
//!
//! The messaging dispatcher slot (`LateBound<dyn MessagingDispatcher>`) is
//! seeded unbound: the dispatcher needs the API's agent proxy, which exists
//! only after this state is built, so the API binds it while it builds its
//! routes and the chat-platform surfaces read the slot at dispatch time.
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
//! [`ExtensionContext::extension_state`]: systemprompt_extension::ExtensionContext::extension_state
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
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::{ContentConfigRaw, LateBound};
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::EscalationDelivery;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::{MessagingDispatcher, SecretStore};

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
    pub system_admin: &'a SystemAdmin,
    pub event_router: &'a EventRouter,
    pub documents: DocumentsHandlesSlot,
    #[cfg(feature = "notifications")]
    pub escalation: EscalationDelivery,
}

/// The handles only the API seeds on top of [`KernelSeed`].
pub struct ApiKernelSeed<'a> {
    pub base: KernelSeed<'a>,
    pub oauth: &'a Arc<OAuthRepositories>,
    pub ai: &'a Arc<AiRepositories>,
    pub ai_service: Option<&'a Arc<AiService>>,
    pub geoip: Option<&'a GeoIpReader>,
    pub content_config: Option<&'a Arc<ContentConfigRaw>>,
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
        .with_sealed(Arc::clone(seed.config))
        .with(seed.services)
        .with_sealed(seed.services_root)
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
        .with(Arc::clone(seed.oauth))
        .with(Arc::clone(seed.ai))
        .with(LateBound::<dyn MessagingDispatcher>::default());
    match seed.ai_service {
        Some(ai) => state.insert(Arc::clone(ai)),
        None => state.reserve::<Arc<AiService>>(),
    }
    match seed.geoip {
        Some(reader) => state.insert(reader.clone()),
        None => state.reserve::<GeoIpReader>(),
    }
    match seed.content_config {
        Some(config) => state.insert(Arc::clone(config)),
        None => state.reserve::<Arc<ContentConfigRaw>>(),
    }
    state
}
