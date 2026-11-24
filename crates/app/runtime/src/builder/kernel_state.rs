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
//! child host, which delivers through the serving host's consumer), and the
//! names of the roles the admitted members declare (`CatalogRoleNames`).
//! The authz hook and the document store are built from the resolved state,
//! so neither is seeded; the read half of each cell is (`AuthzHookCell`
//! sealed, `DocumentsHandlesSlot` open); the host keeps each bind half and
//! binds it once the value is built (the store from the `documents` member).
//!
//! The provider-key lookup (`Arc<dyn SecretLookup>`: the whole secrets
//! document on the API, the granted set on a child host) and the connector
//! token cache (`Arc<TokenCache>`, shared by the proxy, the gateway engines
//! and the AI service) are sealed seeds on every host: they resolve and cache
//! provider credentials.
//!
//! The runtime `Config`, the secret store, the system admin, the active
//! services root, the profile's services bundle sources (`BundleSources`: the
//! pinned sources, bundle cache and base tree, never another profile value),
//! the profile's referenced secret names (`ProfileSecretReferences`, which the
//! kernel's install-secret refusal reads) and the event router are sealed, and
//! so are the JWT signing keys the API seeds: kernel code and a core member's
//! provide fns, factories and slot builds read them; an install member's
//! provide fns, factories and slot builds read only the rows of
//! `systemprompt_extension::reserved::INSTALL_SEEDS`; no route set's state
//! reads one; and every context handed to member runtime code (a job, a runner
//! service, a config plane, a member command) is built without them. `Config`,
//! the secret store and the signing keys carry credentials; the system admin,
//! the services root and the bundle sources are boot-only identity and layout;
//! the event router publishes to and subscribes on every user's stream. Every
//! other seed is open. A member whose runtime code needs a capability derived
//! from a sealed seed builds a narrow handle in its provide fn or factory.
//!
//! The API also seeds the one `Arc<dyn TokenAuthority>` built over its signing
//! keys and issuer, sealed and recorded under the `identity-token-authority`
//! conferral row; `AppContext::token_authority` hands out the same value. The
//! signing keys are recorded under the `identity-signing-keys` row: both are
//! conferred on `identity-http`, the issuer. The OAuth repositories
//! (`Arc<OAuthRepositories>`: clients, authorization codes, refresh tokens,
//! passkeys) are sealed the same way under the `identity-oauth-repositories`
//! row, so no install's provide fn or route state reads them; the agent-runs
//! member's runner factory reads them as a core member's factory. A child host
//! holds verification keys only, so it seeds no token authority and no OAuth
//! repositories.
//!
//! Every host seeds its verification keyring (`VerificationKeyring`, public
//! keys only: the API's from its signing keys, a child host's from the
//! verification keys it was handed), open; the request authenticator verifies
//! against it.
//!
//! Every host seeds the token revocation check (`DynTokenRevocations`, the
//! jti table behind its cache) open; the users member builds the request
//! authenticator over it, so a child host's agent server refuses a revoked
//! token as the API does.
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
use systemprompt_governance::authz::AuthzDecisionHook;
use systemprompt_governance::policy::secrets::SecretScanner;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::profile::ProfileSecretReferences;
use systemprompt_models::a2a::dispatch::AgentDispatch;
use systemprompt_models::{ContentConfigRaw, late_pair};

use super::kernel_binders::{ApiKernelBinders, ApiLateBinders, KernelBinders};
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::DynTokenRevocations;
use systemprompt_oauth::repository::JtiRevocationChecker;
use systemprompt_provider_contracts::{Conferred, Dependencies};
use systemprompt_traits::MessagingDispatcher;

pub use super::kernel_seed::{ApiKernelSeed, KernelCoordination, KernelSeed};

pub fn base_kernel_state(seed: KernelSeed<'_>) -> (Dependencies, KernelBinders) {
    let (scanner, secret_scanner) = late_pair::<SecretScanner>();
    let (authz_cell, authz_hook) = late_pair::<dyn AuthzDecisionHook>();
    let revocations: DynTokenRevocations = Arc::new(JtiRevocationChecker::new(
        seed.database,
        seed.cache_epochs.view(),
    ));
    let state = Dependencies::new()
        .with(scanner)
        .with(revocations)
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
        .with(seed.verification.clone())
        .with(seed.services)
        .with_sealed(seed.services_root)
        .with_sealed(seed.profile_path)
        .with_sealed(BundleSources::of(seed.profile))
        .with_sealed(ProfileSecretReferences::of(seed.profile))
        .with_sealed(seed.secrets)
        .with_sealed(seed.secret_lookup)
        .with_sealed(seed.connector_tokens)
        .with_sealed(seed.system_admin.clone())
        .with_sealed(Arc::new(seed.event_router.clone()))
        .with_sealed(authz_cell)
        .with(Arc::new(seed.catalog_roles))
        .with(seed.documents)
        .with(seed.escalation);
    (
        state,
        KernelBinders {
            secret_scanner,
            authz_hook,
        },
    )
}

pub fn kernel_state(seed: ApiKernelSeed<'_>) -> (Dependencies, ApiKernelBinders) {
    let (agent_dispatch_cell, agent_dispatch) = late_pair::<dyn AgentDispatch>();
    let (messaging_cell, messaging) = late_pair::<dyn MessagingDispatcher>();
    let (base, base_binders) = base_kernel_state(seed.base);
    let mut state = base
        .with_sealed_conferred(
            seed.signing_keys.clone(),
            Conferred {
                holder: ExtensionId::new(ConferralRow::IdentitySigningKeys.entry().holder.id()),
                row: ConferralRow::IdentitySigningKeys.name(),
            },
        )
        .with_sealed_conferred(
            Arc::clone(seed.token_authority),
            Conferred {
                holder: ExtensionId::new(ConferralRow::IdentityTokenAuthority.entry().holder.id()),
                row: ConferralRow::IdentityTokenAuthority.name(),
            },
        )
        .with_sealed(Arc::clone(seed.spawner))
        .with_sealed(seed.child_secrets)
        .with(seed.signing_keys.verification_keys().clone())
        .with_sealed_conferred(
            Arc::clone(seed.oauth),
            Conferred {
                holder: ExtensionId::new(
                    ConferralRow::IdentityOAuthRepositories.entry().holder.id(),
                ),
                row: ConferralRow::IdentityOAuthRepositories.name(),
            },
        )
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
