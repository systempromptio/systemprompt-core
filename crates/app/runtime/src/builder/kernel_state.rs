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
//! The provider-key lookup (`Arc<dyn SecretLookup>`: the whole secrets
//! document on the API, the granted set on a child host) and the connector
//! token cache (`Arc<TokenCache>`, shared by the proxy, the gateway engines
//! and the AI service) are sealed seeds on every host: they resolve and cache
//! provider credentials.
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
//! The API also seeds the one `Arc<dyn TokenAuthority>` built over its signing
//! keys and issuer, sealed and recorded under the `identity-token-authority`
//! conferral row; `AppContext::token_authority` hands out the same value. A
//! child host holds verification keys only, so it seeds no token authority.
//!
//! The API seeds the token revocation check (`DynTokenRevocations`, the jti
//! table behind its cache) open; the users member builds the request
//! authenticator over it.
//!
//! The cache epochs are seeded twice: the advancing `CacheEpochs` handle
//! sealed (the event relay and kernel code advance it), and its read half
//! `CacheEpochView` open, which every cache that only reads an epoch takes.
//!
//! Every late-bound cell is split ([`late_pair`]): the state holds only the
//! read half, and the write half is returned beside the state
//! ([`KernelBinders`], [`ApiKernelBinders`]) for the one kernel function
//! that binds it, so no member can bind a cell first. The secret scanner's
//! read half (`SecretScannerCell`) is open: reading the configured scanner
//! confers nothing. The messaging dispatcher (`LateRead<dyn
//! MessagingDispatcher>`) and the agent dispatch (`AgentDispatchCell`) are
//! seeded unbound and **sealed**: both need the API's agent proxy, which
//! exists only after this state is built, so the API binds them while it
//! builds its routes. The dispatcher mints a token for the user it resolves
//! and sends as them, so only kernel code and provide fns read it; the Slack
//! and Teams members wrap it, in their own provide fns, in a handle that
//! dispatches only a request their verifier has verified.
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

use systemprompt_analytics::GeoIpReader;
use systemprompt_config::DynChildSecretEnv;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_governance::policy::secrets::SecretScanner;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::{ActiveProfilePath, ActiveServicesRoot};
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::a2a::dispatch::AgentDispatch;
use systemprompt_models::{ContentConfigRaw, late_pair};

use super::kernel_binders::{ApiKernelBinders, ApiLateBinders, KernelBinders};
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::{DynTokenRevocations, TokenAuthority};
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::EscalationDelivery;
use systemprompt_oauth::repository::{JtiRevocationChecker, OAuthRepositories};
use systemprompt_provider_contracts::{Conferred, Dependencies};
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    BackgroundTasks, CacheEpochs, DynAdvisoryLocks, DynInstanceRegistry, DynJwtValidationProvider,
    DynLeaseRepository, FileStorage, MessagingDispatcher, SecretLookup, SecretStore,
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
    #[cfg(feature = "notifications")]
    pub escalation: EscalationDelivery,
}

/// The handles only the API seeds on top of [`KernelSeed`].
pub struct ApiKernelSeed<'a> {
    pub base: KernelSeed<'a>,
    pub signing_keys: &'a SigningKeys,
    pub token_authority: &'a Arc<dyn TokenAuthority>,
    pub spawner: &'a Arc<Spawner>,
    pub child_secrets: DynChildSecretEnv,
    pub oauth: &'a Arc<OAuthRepositories>,
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
            .finish_non_exhaustive()
    }
}

pub fn base_kernel_state(seed: KernelSeed<'_>) -> (Dependencies, KernelBinders) {
    let (scanner, secret_scanner) = late_pair::<SecretScanner>();
    let state = Dependencies::new()
        .with(scanner)
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
        .with_sealed(seed.cache_epochs.clone())
        .with(seed.cache_epochs.view())
        .with(seed.services)
        .with_sealed(seed.services_root)
        .with_sealed(seed.profile_path)
        .with_sealed(BundleSources::of(seed.profile))
        .with_sealed(seed.secrets)
        .with_sealed(seed.secret_lookup)
        .with_sealed(seed.connector_tokens)
        .with_sealed(seed.system_admin.clone())
        .with_sealed(Arc::new(seed.event_router.clone()))
        .with(seed.documents);
    #[cfg(feature = "notifications")]
    let state = state.with(seed.escalation);
    (state, KernelBinders { secret_scanner })
}

fn jwt_validation(config: &Config, keys: &SigningKeys) -> DynJwtValidationProvider {
    Arc::new(systemprompt_oauth::JwtValidationProviderImpl::from_config(
        config,
        keys.keyring().clone(),
    ))
}

pub fn kernel_state(seed: ApiKernelSeed<'_>) -> (Dependencies, ApiKernelBinders) {
    let jwt = jwt_validation(seed.base.config, seed.signing_keys);
    let (agent_dispatch_cell, agent_dispatch) = late_pair::<dyn AgentDispatch>();
    let (messaging_cell, messaging) = late_pair::<dyn MessagingDispatcher>();
    let revocations: DynTokenRevocations = Arc::new(JtiRevocationChecker::from_repository(
        seed.oauth.oauth.clone(),
        seed.base.cache_epochs.view(),
    ));
    let (base, base_binders) = base_kernel_state(seed.base);
    let mut state = base
        .with_sealed(seed.signing_keys.clone())
        .with_sealed_conferred(
            Arc::clone(seed.token_authority),
            Conferred {
                holder: ExtensionId::new(ConferralRow::IdentityTokenAuthority.entry().holder.id()),
                row: ConferralRow::IdentityTokenAuthority.name(),
            },
        )
        .with_sealed(Arc::clone(seed.spawner))
        .with_sealed(seed.child_secrets)
        .with(jwt)
        .with(seed.signing_keys.keyring().clone())
        .with(seed.signing_keys.verification_keys().clone())
        .with(Arc::clone(seed.oauth))
        .with(revocations)
        .with_sealed(messaging_cell)
        .with_sealed(agent_dispatch_cell);
    match seed.geoip {
        Some(reader) => state.insert(Arc::clone(reader)),
        None => state.reserve::<GeoIpReader>(),
    }
    match seed.content_config {
        Some(config) => state.insert(Arc::clone(config)),
        None => state.reserve::<Arc<ContentConfigRaw>>(),
    }
    (
        state,
        ApiKernelBinders {
            base: base_binders,
            late: ApiLateBinders {
                agent_dispatch,
                messaging,
            },
        },
    )
}
