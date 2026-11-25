//! The kernel's own entries in the extension state: the handles the host
//! seeds the state with before any extension provides. [`base_kernel_state`]
//! holds the entries every process seeds, the API and a child host alike,
//! from the handles in a [`KernelSeed`]; [`kernel_state`] adds what only the
//! API composes. An extension reads them from the same map as every other
//! extension's values, and cannot provide a type the kernel already holds.
//!
//! Every host seeds the database handle, the paths, the files, notifications
//! and brain profiles, the profile's `UnappliedPlaneMode`, the instance id, the
//! file storage (`Arc<dyn FileStorage>`), the background-task owner
//! (`BackgroundTasks`), the coordination seams (`DynInstanceRegistry`,
//! `DynLeaseRepository`, `DynAdvisoryLocks`; a child host seeds
//! `UnavailableCoordination` for all three), the runtime `Config`, the services
//! view and active services root, the secret store, the system admin, the
//! event router and the escalation delivery mode the notifications member
//! assembles its service with (`InProcess` on the API, `Consumer` on a
//! child host, which delivers through the serving host's consumer), and the
//! names of the roles the admitted members declare (`CatalogRoleNames`).
//! The authz hook is built from the resolved state, so it is not seeded; the
//! read half of its cell is (`AuthzHookCell`, sealed), and the host keeps the
//! bind half and binds it once the hook is built. The document store cell is
//! the files member's declared late cell, which the kernel seeds and binds.
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
//! Every host also seeds the three install views, sealed, each copied here
//! from the profile, the runtime `Config` and the services view: the
//! `ConfigView` (no database URL, no signing key path), the `ProfileView`
//! (no OTLP credentials) and the `InstallServicesView` (no source
//! credentials, no reload handle). They are the rows of `INSTALL_SEEDS` next
//! to the system admin; no route set's state reads them. Only the API seeds
//! conferral issuers: the member cipher issuer when its secret store holds
//! the `encryption_master_key`, the share-token issuer when it holds the
//! `manifest_signing_secret_seed`, and the scoped token minter issuer over
//! its token authority; it reserves `ConferralsWithheld`. The issuers are
//! sealed and no member reads one: a member reads its own handle
//! (`INSTALL_CONFERRALS`). A child host seeds no issuer, even when the
//! master key was granted to it, and seeds `ConferralsWithheld` sealed, so a
//! member's declared read there is withheld rather than missing.
//!
//! The API also seeds the one `Arc<dyn TokenAuthority>` built over its signing
//! keys and issuer, sealed and recorded under the `identity-token-authority`
//! conferral row; host code reads the same value from the kernel state. The
//! signing keys are recorded under the `identity-signing-keys` row: both are
//! conferred on `identity-http`, the issuer. The API seeds the OAuth
//! at-rest pepper (`AtRestPepper`) sealed; the oauth member builds its
//! repositories over it in its own provide and confers them on
//! `identity-http`. A child host holds verification keys only, so it seeds no
//! token authority and no pepper, and the oauth member withholds its
//! repositories there.
//!
//! Every host seeds its verification keyring (`VerificationKeyring`, public
//! keys only: the API's from its signing keys, a child host's from the
//! verification keys it was handed), open; the request authenticator verifies
//! against it.
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
//! seeded unbound and **sealed**. The messaging cell is bound by the context
//! builder as soon as the kernel resolves, to the dispatcher the `agent-http`
//! member provides; the agent dispatch needs the API's agent proxy, which
//! exists only after the context is built, so the API binds it while it
//! builds its routes. The dispatcher mints a token for the user it resolves
//! and sends as them, so only kernel code and provide fns read it; the Slack
//! and Teams members wrap it, in their own provide fns, in a handle that
//! dispatches only a request their verifier has verified.
//!
//! The API also seeds the loaded content configuration
//! (`Arc<ContentConfigRaw>`, whose `ContentRouting` resolves a page URL to a
//! slug). It is optional: when it is not configured its type is reserved, so a
//! reader's `find` is `None` (`get` is a typed `MissingDependency`) and no
//! extension can `provide` a substitute. The analytics repositories and the
//! `GeoIP` database are not seeded: the analytics member opens and provides
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

use systemprompt_governance::authz::AuthzDecisionHook;
use systemprompt_governance::policy::secrets::SecretScanner;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::profile::ProfileSecretReferences;
use systemprompt_models::a2a::dispatch::AgentDispatch;
use systemprompt_models::{ContentConfigRaw, late_pair};

use super::conferred::{KernelTokenMinters, with_secret_issuers};
use super::kernel_binders::{ApiKernelBinders, ApiLateBinders, KernelBinders};
use systemprompt_extension::ConferralsWithheld;
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_governance::authz::GovernanceDecisionRepository;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::ConfigView;
use systemprompt_manifest::profile::ProfileView;
use systemprompt_manifest::services::InstallServicesView;
use systemprompt_models::auth::ScopedTokenMinter;
use systemprompt_provider_contracts::{Conferred, Dependencies};
use systemprompt_traits::{DynConferralIssuer, MessagingDispatcher};

pub use super::kernel_seed::{ApiKernelSeed, KernelCoordination, KernelSeed};

pub fn base_kernel_state(seed: KernelSeed<'_>) -> (Dependencies, KernelBinders) {
    let (scanner, secret_scanner) = late_pair::<SecretScanner>();
    let install_services =
        InstallServicesView::of(seed.services.clone(), &seed.services_root, seed.profile);
    let (authz_cell, authz_hook) = late_pair::<dyn AuthzDecisionHook>();
    let state = Dependencies::new()
        .with(scanner)
        .with(Arc::clone(seed.database))
        .with(Arc::clone(seed.app_paths))
        .with(Arc::new(seed.profile.notifications))
        .with(Arc::new(seed.profile.brain.clone()))
        .with(seed.profile.unapplied_plane_mode())
        .with(seed.config.instance_id.clone())
        .with(seed.file_storage)
        .with(seed.background_tasks)
        .with(seed.coordination.instances)
        .with(seed.coordination.leases)
        .with(seed.coordination.locks)
        .with_sealed(Arc::clone(seed.config))
        .with_sealed(Arc::new(ConfigView::of(seed.config)))
        .with_sealed(Arc::new(ProfileView::of(seed.profile)))
        .with_sealed(install_services)
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
        .with(seed.escalation)
        .with_sealed(seed.audit_spans);
    (
        state,
        KernelBinders {
            secret_scanner,
            authz_hook,
        },
    )
}

fn minter_issuer(seed: &ApiKernelSeed<'_>) -> DynConferralIssuer<dyn ScopedTokenMinter> {
    Arc::new(KernelTokenMinters::new(
        Arc::clone(seed.token_authority),
        seed.base.verification.clone(),
        seed.base.config.jwt_issuer.clone(),
        GovernanceDecisionRepository::from_pool(seed.base.database.write_pool()),
        seed.base.system_admin.id().clone(),
    ))
}

pub fn kernel_state(seed: ApiKernelSeed<'_>) -> (Dependencies, ApiKernelBinders) {
    let (agent_dispatch_cell, agent_dispatch) = late_pair::<dyn AgentDispatch>();
    let (messaging_cell, messaging) = late_pair::<dyn MessagingDispatcher>();
    let minter = minter_issuer(&seed);
    let secrets = Arc::clone(&seed.base.secrets);
    let (base, base_binders) = base_kernel_state(seed.base);
    let mut state = with_secret_issuers(base, &*secrets)
        .reserving::<ConferralsWithheld>()
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
        .with_sealed(seed.at_rest_pepper)
        .with_sealed(minter)
        .with_sealed(messaging_cell)
        .with_sealed(agent_dispatch_cell);
    match seed.content_config {
        Some(config) => state.insert(Arc::clone(config)),
        None => state.reserve::<Arc<ContentConfigRaw>>(),
    }
    (
        state,
        ApiKernelBinders {
            base: base_binders,
            messaging,
            late: ApiLateBinders { agent_dispatch },
        },
    )
}
