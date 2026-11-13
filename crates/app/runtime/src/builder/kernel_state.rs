//! The kernel's own entries in the extension state: the handles the host
//! seeds the state with before any extension provides. An extension reads
//! them from the same map as every other extension's values, and cannot
//! provide a type the kernel already holds.
//!
//! The process's one `AiService` (present only when inference is configured)
//! and its one `ArtifactIngest` are composed here from several domains, so the
//! composition root seeds them; an extension router reads them from
//! [`ExtensionContext::extension_state`] and an absent `AiService` is a typed
//! `MissingDependency`, never a fallback service.
//!
//! [`ExtensionContext::extension_state`]: systemprompt_extension::ExtensionContext::extension_state
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::Dependencies;

use crate::context::AppContext;

pub fn kernel_state(app: &AppContext) -> Dependencies {
    let mut state = Dependencies::new()
        .with(Arc::clone(app.db_pool()))
        .with(Arc::clone(app.app_paths_arc()))
        .with(Arc::clone(app.files_config()))
        .with(Arc::new(app.profile().notifications))
        .with(Arc::clone(app.oauth_repositories()))
        .with(Arc::clone(app.ai_repositories()))
        .with(Arc::clone(app.analytics_repositories()))
        .with(app.artifact_ingest_arc());
    if let Some(ai) = &app.subsystems.ai_service {
        state.insert(Arc::clone(ai));
    }
    if let Some(reader) = app.geoip_reader() {
        state.insert(Arc::clone(reader));
    }
    if app.files_config().documents_config().is_enabled() {
        state.insert(Arc::clone(app.documents().service()));
    }
    #[cfg(feature = "brain")]
    {
        state.insert(Arc::new(app.profile().brain.clone()));
        state.insert::<systemprompt_extension::DynNotificationSink>(Arc::clone(
            &app.notifications().sink,
        ));
    }
    state
}
