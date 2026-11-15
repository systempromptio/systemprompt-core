//! The kernel's own entries in the extension state: the handles the host
//! seeds the state with before any extension provides. [`base_kernel_state`]
//! holds the entries every process seeds, the API and a child host alike;
//! [`kernel_state`] adds what only the API composes. An extension reads
//! them from the same map as every other extension's values, and cannot
//! provide a type the kernel already holds.
//!
//! The process's one `AiService` (present only when inference is configured)
//! and its one `ArtifactIngest` are composed here from several domains, so the
//! composition root seeds them; an extension router reads them from
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
//! from, so a capability that owns no composition step reads them like any
//! other state entry:
//!
//! - `Arc<AnalyticsRepositories>`: the analytics repositories, session store
//!   included, which the kernel composes because the session store and the
//!   `AnalyticsProvider` sit on the authentication path the kernel builds
//!   before any extension provides;
//! - `Arc<AnalyticsBroadcaster>`: the analytics event broadcaster only, never
//!   the other user event streams;
//! - `Arc<ContentConfigRaw>`: the loaded content configuration (its
//!   `ContentRouting` resolves a page URL to a slug);
//! - `GeoIpReader`: the configured `GeoIP` database.
//!
//! The content configuration and the `GeoIP` reader are optional: when either
//! is not configured its type is reserved, so a reader's `find` is `None`
//! (`get` is a typed `MissingDependency`) and no extension can `provide` a
//! substitute.
//!
//! [`ExtensionContext::extension_state`]: systemprompt_extension::ExtensionContext::extension_state
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_analytics::GeoIpReader;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_files::FilesConfig;
use systemprompt_manifest::Profile;
use systemprompt_models::{ContentConfigRaw, LateBound};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::MessagingDispatcher;

use crate::context::AppContext;
use crate::documents::DocumentsRuntime;

pub fn base_kernel_state(
    database: &DbPool,
    app_paths: &Arc<AppPaths>,
    files_config: &Arc<FilesConfig>,
    profile: &Profile,
    documents: &DocumentsRuntime,
) -> Dependencies {
    let mut state = Dependencies::new()
        .with(Arc::clone(database))
        .with(Arc::clone(app_paths))
        .with(Arc::clone(files_config))
        .with(Arc::new(profile.notifications));
    if files_config.documents_config().is_enabled() {
        state.insert(Arc::clone(documents.service()));
    }
    state
}

pub fn kernel_state(app: &AppContext) -> Dependencies {
    let mut state = base_kernel_state(
        app.db_pool(),
        app.app_paths_arc(),
        app.files_config(),
        app.profile(),
        app.documents(),
    )
    .with(Arc::clone(app.oauth_repositories()))
    .with(Arc::clone(app.ai_repositories()))
    .with(Arc::clone(app.analytics_repositories()))
    .with(Arc::clone(&app.event_broadcasters().analytics))
    .with(app.artifact_ingest_arc())
    .with(LateBound::<dyn MessagingDispatcher>::default());
    match &app.subsystems.ai_service {
        Some(ai) => state.insert(Arc::clone(ai)),
        None => state.reserve::<Arc<systemprompt_ai::AiService>>(),
    }
    match app.geoip_reader() {
        Some(reader) => state.insert(Arc::clone(reader)),
        None => state.reserve::<GeoIpReader>(),
    }
    match &app.cfg.content_config {
        Some(config) => state.insert(Arc::clone(config)),
        None => state.reserve::<Arc<ContentConfigRaw>>(),
    }
    #[cfg(feature = "brain")]
    state.insert(Arc::new(app.profile().brain.clone()));
    state
}
